//! Keep Media2 values native throughout editing; retain Media1-only fields on save.
use super::*;
use oxvif::{FloatRange, IntRange, Resolution, VideoEncoderOptions2};

#[derive(Clone, Debug, Default)]
pub struct EncoderOptions {
    pub resolutions: Vec<Resolution>,
    pub frame_rates: Vec<f32>,
    pub frame_rate_range: Option<IntRange>,
    pub bitrate_range: Option<IntRange>,
    pub gov_length_range: Option<IntRange>,
    pub quality_range: Option<FloatRange>,
    pub profiles: Vec<String>,
}

impl From<VideoEncoderOptions2> for EncoderOptions {
    fn from(o: VideoEncoderOptions2) -> Self {
        Self {
            resolutions: o.resolutions,
            frame_rates: o
                .frame_rates
                .into_iter()
                .filter(|f| f.is_finite() && *f >= 0.0)
                .collect(),
            bitrate_range: o.bitrate_range,
            gov_length_range: o.gov_length_range,
            quality_range: o.quality_range,
            profiles: o.profiles,
            ..Default::default()
        }
    }
}

impl EncoderOptions {
    fn media1(encoding: &VideoEncoding, o: VideoEncoderConfigurationOptions) -> Self {
        let mut result = Self {
            quality_range: o.quality_range,
            ..Default::default()
        };
        match encoding {
            VideoEncoding::H264 => {
                if let Some(h) = o.h264 {
                    result.resolutions = h.resolutions;
                    result.frame_rate_range = h.frame_rate_range;
                    result.bitrate_range = h.bitrate_range;
                    result.gov_length_range = h.gov_length_range;
                    result.profiles = h.profiles;
                }
            }
            VideoEncoding::H265 => {
                if let Some(h) = o.h265 {
                    result.resolutions = h.resolutions;
                    result.frame_rate_range = h.frame_rate_range;
                    result.bitrate_range = h.bitrate_range;
                    result.gov_length_range = h.gov_length_range;
                    result.profiles = h.profiles;
                }
            }
            VideoEncoding::Jpeg => {
                if let Some(j) = o.jpeg {
                    result.resolutions = j.resolutions;
                    result.frame_rate_range = j.frame_rate_range;
                }
            }
            _ => {}
        }
        result
    }
}

#[derive(Clone, Debug)]
pub struct EncoderSettings {
    pub config: VideoEncoderConfiguration2,
    pub options: EncoderOptions,
    // Presence also records which service loaded this configuration. Never
    // retry a failed Media2 write through Media1 or invent a conversion target.
    pub(crate) media1: Option<VideoEncoderConfiguration>,
}

impl EncoderSettings {
    pub fn is_media2(&self) -> bool {
        self.media1.is_none()
    }

    /// Validate without truncating. An unchanged current value may be absent
    /// from a camera's advertised list and must remain saveable.
    pub fn parse_frame_rate(&self, text: &str) -> Option<f32> {
        let value = text.trim().parse::<f32>().ok()?;
        if !value.is_finite() || value < 0.0 {
            return None;
        }
        if !self.is_media2() && (value.fract() != 0.0 || f64::from(value) > f64::from(u32::MAX)) {
            return None;
        }
        if self
            .config
            .rate_control
            .as_ref()
            .is_some_and(|r| r.frame_rate_limit == value)
        {
            return Some(value);
        }
        if self
            .options
            .frame_rate_range
            .is_some_and(|range| value < range.min as f32 || value > range.max as f32)
        {
            return None;
        }
        if !self.options.frame_rates.is_empty() && !self.options.frame_rates.contains(&value) {
            return None;
        }
        Some(value)
    }

    fn media1_for_save(&self) -> Result<VideoEncoderConfiguration, ApiError> {
        let mut original = self.media1.clone().ok_or("Missing Media1 configuration")?;
        let cfg = &self.config;
        original.resolution = cfg.resolution;
        original.quality = cfg.quality;
        if let (Some(dst), Some(src)) = (&mut original.rate_control, &cfg.rate_control) {
            // Guard again at the API boundary; callers need not be a form.
            if !src.frame_rate_limit.is_finite()
                || src.frame_rate_limit < 0.0
                || src.frame_rate_limit.fract() != 0.0
                || f64::from(src.frame_rate_limit) > f64::from(u32::MAX)
            {
                return Err("Media1 frame rate must be a nonnegative integer".into());
            }
            dst.frame_rate_limit = src.frame_rate_limit as u32;
            dst.bitrate_limit = src.bitrate_limit;
        }
        match original.encoding {
            VideoEncoding::H264 => {
                if let Some(h) = &mut original.h264 {
                    if let Some(g) = cfg.gov_length {
                        h.gov_length = g;
                    }
                    if let Some(p) = &cfg.profile {
                        h.profile.clone_from(p);
                    }
                }
            }
            VideoEncoding::H265 => {
                if let Some(h) = &mut original.h265 {
                    if let Some(g) = cfg.gov_length {
                        h.gov_length = g;
                    }
                    if let Some(p) = &cfg.profile {
                        h.profile.clone_from(p);
                    }
                }
            }
            _ => {}
        }
        Ok(original)
    }
}

/// Read the selected profile and its encoder from the same service. Media2
/// profile/configuration tokens must never be inferred from Media1 tokens.
#[instrument(skip(creds), fields(addr, profile_token))]
pub async fn get_encoder_settings(
    addr: &str,
    creds: &Credentials,
    profile_token: Option<&str>,
) -> Result<(ChannelPick, EncoderSettings), ApiError> {
    let s = session_for(addr, creds).await?;
    if s.capabilities().media2.url.is_some() {
        let profiles = s.get_profiles_media2().await.map_err(|e| e.to_string())?;
        let selected = profile_token.and_then(|t| {
            profiles
                .iter()
                .find(|p| p.token == t && p.video_encoder_token.is_some())
        });
        let profile = selected
            .or_else(|| profiles.iter().find(|p| p.video_encoder_token.is_some()))
            .ok_or("no_encoder")?;
        let token = profile.video_encoder_token.as_deref().ok_or("no_encoder")?;
        let config = s
            .get_video_encoder_configuration_media2(token)
            .await
            .map_err(|e| e.to_string())?;
        let options = s
            .get_video_encoder_configuration_options_media2(token)
            .await
            .ok()
            .and_then(|o| {
                o.options
                    .into_iter()
                    .find(|o| o.encoding == config.encoding)
            })
            .map(EncoderOptions::from)
            .unwrap_or_default();
        return Ok((
            ChannelPick {
                token: token.into(),
                fell_back: profile_token.is_some() && selected.is_none(),
            },
            EncoderSettings {
                config,
                options,
                media1: None,
            },
        ));
    }
    let profiles = s.get_profiles().await.map_err(|e| e.to_string())?;
    let pick = pick_channel(&profiles, profile_token, ChannelKind::Encoder).ok_or("no_encoder")?;
    let original = get_video_encoder_configuration(addr, creds, &pick.token).await?;
    let options = get_video_encoder_configuration_options(addr, creds, &pick.token)
        .await
        .ok()
        .map(|o| EncoderOptions::media1(&original.encoding, o))
        .unwrap_or_default();
    Ok((
        pick,
        EncoderSettings {
            config: to_video_encoder_configuration2(&original),
            options,
            media1: Some(original),
        },
    ))
}

#[instrument(skip(creds, settings), fields(addr))]
pub async fn save_encoder_settings(
    addr: &str,
    creds: &Credentials,
    settings: &EncoderSettings,
) -> Result<(), ApiError> {
    if settings.is_media2() {
        let s = session_for(addr, creds).await?;
        trace_result(
            "SetVideoEncoderConfiguration2",
            addr,
            s.set_video_encoder_configuration_media2(&settings.config)
                .await,
        )
    } else {
        set_video_encoder_configuration(addr, creds, &settings.media1_for_save()?).await
    }
}

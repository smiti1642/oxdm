pub(crate) mod en;
pub(crate) mod ru;
pub(crate) mod zh_tw;

use crate::state::Locale;

/// Look up a translated string by key for the given locale.
/// Falls back to English if the key is missing in the target locale.
/// Panics (in debug) / returns the key itself if missing everywhere.
pub fn t(locale: Locale, key: &str) -> &'static str {
    let result = match locale {
        Locale::En => en::get(key),
        Locale::ZhTw => zh_tw::get(key),
        Locale::Ru => ru::get(key),
    };
    // Fallback to English
    result.or_else(|| en::get(key)).unwrap_or_else(|| {
        debug_assert!(false, "missing i18n key: {key}");
        // In release, just return the key so the UI doesn't break
        // Safety: key is &str but we need &'static str — leak is acceptable
        // for a small number of missing keys in production.
        Box::leak(key.to_string().into_boxed_str())
    })
}

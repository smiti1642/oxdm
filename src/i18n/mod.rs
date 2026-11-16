pub(crate) mod en;
pub(crate) mod ru;
pub(crate) mod zh_tw;

use crate::state::Locale;

pub(crate) fn console_command(locale: Locale, id: &str) -> Option<&'static str> {
    match locale {
        Locale::ZhTw => zh_tw::console_command(id),
        Locale::Ru => ru::console_command(id),
        Locale::En => None,
    }
}

pub(crate) fn console_option(locale: Locale, id: &str) -> Option<&'static str> {
    match locale {
        Locale::ZhTw => zh_tw::console_option(id),
        Locale::Ru => ru::console_option(id),
        Locale::En => en::console_option(id),
    }
}

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

use crate::state::{Credentials, Locale, Theme};

#[test]
fn theme_cycles_correctly() {
    assert_eq!(Theme::Dark.next(), Theme::Light);
    assert_eq!(Theme::Light.next(), Theme::Classic);
    assert_eq!(Theme::Classic.next(), Theme::Dark);
}

#[test]
fn theme_css_class_contains_theme_name() {
    assert!(Theme::Dark.css_class().contains("dark"));
    assert!(Theme::Light.css_class().contains("light"));
    assert!(Theme::Classic.css_class().contains("classic"));
}

#[test]
fn locale_cycles_correctly() {
    assert_eq!(Locale::En.next(), Locale::ZhTw);
    assert_eq!(Locale::ZhTw.next(), Locale::Ru);
    assert_eq!(Locale::Ru.next(), Locale::En);
}

#[test]
fn locale_labels_are_short() {
    for locale in [Locale::En, Locale::ZhTw, Locale::Ru] {
        let label = locale.label();
        assert!(label.len() <= 4, "label too long: {label}");
    }
}

#[test]
fn credentials_default_is_empty() {
    let c = Credentials::default();
    assert!(c.username.is_empty());
    assert!(c.password.is_empty());
}

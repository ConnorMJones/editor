use model::color::Rgb;
use model::config::{Config, Restore};

#[test]
fn empty_config_uses_defaults() {
    let config: Config = toml::from_str("").expect("empty config parses");
    assert_eq!(config.session.restore, Restore::Off);
    assert_eq!(config.theme.background, None);
}

#[test]
fn overrides_background_and_session() {
    let config: Config = toml::from_str(
        r##"
        [theme]
        background = "#004040"

        [session]
        restore = "last"
        "##,
    )
    .expect("config parses");
    assert_eq!(
        config.theme.background,
        Some(Rgb {
            r: 0x00,
            g: 0x40,
            b: 0x40
        })
    );
    assert_eq!(config.session.restore, Restore::Last);
}

#[test]
fn unknown_keys_fail() {
    assert!(toml::from_str::<Config>("[theme]\nbackgrond = \"#004040\"").is_err());
}

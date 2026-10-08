use model::color::Rgb;
use model::config::{Config, OpenAs, Restore, TabsConfig};

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

#[test]
fn tabs_open_from_the_tree_as_previews_by_default() {
    let config: Config = toml::from_str("").expect("empty config parses");
    assert_eq!(
        config.tabs,
        TabsConfig {
            open_from_tree: OpenAs::Preview,
            open_from_picker: OpenAs::Permanent,
        }
    );
}

#[test]
fn tabs_sources_are_configurable() {
    let config: Config = toml::from_str(
        r#"
        [tabs]
        open_from_tree = "permanent"
        open_from_picker = "preview"
        "#,
    )
    .expect("config parses");
    assert_eq!(config.tabs.open_from_tree, OpenAs::Permanent);
    assert_eq!(config.tabs.open_from_picker, OpenAs::Preview);

    let partial: Config =
        toml::from_str("[tabs]\nopen_from_picker = \"preview\"\n").expect("config parses");
    assert_eq!(partial.tabs.open_from_tree, OpenAs::Preview);
}

#[test]
fn tabs_reject_bools_and_unknown_keys() {
    assert!(toml::from_str::<Config>("[tabs]\nopen_from_tree = true").is_err());
    assert!(toml::from_str::<Config>("[tabs]\npreview_from_tree = \"preview\"").is_err());
}

#[test]
fn title_bar_defaults_hidden_and_parses() {
    use model::config::TitleBar;
    let config: Config = toml::from_str("").unwrap();
    assert_eq!(config.window.title_bar, TitleBar::Hidden);
    let config: Config = toml::from_str("[window]\ntitle_bar = \"shown\"").unwrap();
    assert_eq!(config.window.title_bar, TitleBar::Shown);
}

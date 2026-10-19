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

#[test]
fn diff_colours_override_and_reject_unknown_roles() {
    let config: Config =
        toml::from_str("[theme.diff]\nadded = \"#00ff00\"\ntheirs = \"#ff8800\"").unwrap();
    assert_eq!(
        config.theme.diff.added,
        Some(Rgb {
            r: 0,
            g: 0xff,
            b: 0
        })
    );
    assert_eq!(
        config.theme.diff.theirs,
        Some(Rgb {
            r: 0xff,
            g: 0x88,
            b: 0
        })
    );
    assert_eq!(config.theme.diff.removed, None);
    assert!(toml::from_str::<Config>("[theme.diff]\ninserted = \"#00ff00\"").is_err());
}

const DEFAULT_CONFIG: &str = include_str!("../../../../default-config.toml");

#[test]
fn the_example_config_holds_the_defaults() {
    let config: Config = toml::from_str(DEFAULT_CONFIG).expect("default-config.toml parses");
    assert_eq!(config, Config::default());
}

#[test]
fn every_commented_setting_in_the_example_config_is_valid() {
    let uncommented: String = DEFAULT_CONFIG
        .lines()
        .map(|line| match line.strip_prefix("# ") {
            Some(setting)
                if setting.starts_with('[')
                    || setting.split_once(" = ").is_some_and(|(key, _)| {
                        key.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                    }) =>
            {
                setting.replace("#rrggbb", "#004040")
            }
            Some(_) | None => line.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    let config: Config = toml::from_str(&uncommented).expect("uncommented settings parse");
    assert_eq!(
        config.theme.background,
        Some(Rgb {
            r: 0x00,
            g: 0x40,
            b: 0x40
        })
    );
}

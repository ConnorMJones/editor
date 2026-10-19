use std::str::from_utf8;

/// Default built-in languages.toml.
pub fn default_lang_config() -> toml::Value {
    let default_config = include_bytes!("../languages.toml");
    toml::from_str(from_utf8(default_config).unwrap())
        .expect("Could not parse built-in languages.toml to valid toml")
}

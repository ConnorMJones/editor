use crate::syntax::{config::Configuration, Loader};

/// Language configuration based on built-in languages.toml.
pub fn default_lang_config() -> Configuration {
    helix_loader::config::default_lang_config()
        .try_into()
        .expect("Could not deserialize built-in languages.toml")
}

/// Language configuration loader based on built-in languages.toml.
pub fn default_lang_loader() -> Loader {
    Loader::new(default_lang_config()).expect("Could not compile loader for default config")
}

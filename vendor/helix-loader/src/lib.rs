pub mod config;
pub mod grammar;
pub mod workspace_trust;

use helix_stdx::env::current_working_dir;

use etcetera::base_strategy::{choose_base_strategy, BaseStrategy};
use std::path::{Path, PathBuf};

pub const VERSION_AND_GIT_HASH: &str = env!("VERSION_AND_GIT_HASH");

static CONFIG_FILE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

static LOG_FILE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

pub fn initialize_config_file(specified_file: Option<PathBuf>) {
    let config_file = specified_file.unwrap_or_else(default_config_file);
    ensure_parent_dir(&config_file);
    CONFIG_FILE.set(config_file).ok();
}

pub fn initialize_log_file(specified_file: Option<PathBuf>) {
    let log_file = specified_file.unwrap_or_else(default_log_file);
    ensure_parent_dir(&log_file);
    LOG_FILE.set(log_file).ok();
}

pub fn config_dir() -> PathBuf {
    // TODO: allow env var override
    let strategy = choose_base_strategy().expect("Unable to find the config directory!");
    let mut path = strategy.config_dir();
    path.push("helix");
    path
}

pub fn cache_dir() -> PathBuf {
    // TODO: allow env var override
    let strategy = choose_base_strategy().expect("Unable to find the cache directory!");
    let mut path = strategy.cache_dir();
    path.push("helix");
    path
}

pub fn data_dir() -> PathBuf {
    let strategy = choose_base_strategy().expect("Unable to find the data directory!");
    let mut path = strategy.data_dir();
    path.push("helix");
    path
}

pub fn config_file() -> PathBuf {
    CONFIG_FILE.get().map(|path| path.to_path_buf()).unwrap()
}

pub fn log_file() -> PathBuf {
    LOG_FILE.get().map(|path| path.to_path_buf()).unwrap()
}

pub fn workspace_config_file() -> PathBuf {
    find_workspace().0.join(".helix").join("config.toml")
}

pub fn workspace_lang_config_file() -> PathBuf {
    find_workspace().0.join(".helix").join("languages.toml")
}

pub fn lang_config_file() -> PathBuf {
    config_dir().join("languages.toml")
}

pub fn default_log_file() -> PathBuf {
    cache_dir().join("helix.log")
}

/// Merge two TOML documents, merging values from `right` onto `left`
///
/// `merge_depth` sets the nesting depth up to which values are merged instead
/// of overridden.
///
/// When a table exists in both `left` and `right`, the merged table consists of
/// all keys in `left`'s table unioned with all keys in `right` with the values
/// of `right` being merged recursively onto values of `left`.
///
/// `crate::merge_toml_values(a, b, 3)` combines, for example:
///
/// b:
/// ```toml
/// [[language]]
/// name = "toml"
/// language-server = { command = "taplo", args = ["lsp", "stdio"] }
/// ```
/// a:
/// ```toml
/// [[language]]
/// language-server = { command = "/usr/bin/taplo" }
/// ```
///
/// into:
/// ```toml
/// [[language]]
/// name = "toml"
/// language-server = { command = "/usr/bin/taplo" }
/// ```
///
/// thus it overrides the third depth-level of b with values of a if they exist,
/// but otherwise merges their values
pub fn merge_toml_values(left: toml::Value, right: toml::Value, merge_depth: usize) -> toml::Value {
    use toml::Value;

    fn get_name(v: &Value) -> Option<&str> {
        v.get("name").and_then(Value::as_str)
    }

    match (left, right) {
        (Value::Array(mut left_items), Value::Array(right_items)) => {
            if merge_depth > 0 {
                left_items.reserve(right_items.len());
                for rvalue in right_items {
                    let lvalue = get_name(&rvalue)
                        .and_then(|rname| {
                            left_items.iter().position(|v| get_name(v) == Some(rname))
                        })
                        .map(|lpos| left_items.remove(lpos));
                    let mvalue = match lvalue {
                        Some(lvalue) => merge_toml_values(lvalue, rvalue, merge_depth - 1),
                        None => rvalue,
                    };
                    left_items.push(mvalue);
                }
                Value::Array(left_items)
            } else {
                Value::Array(right_items)
            }
        }
        (Value::Table(mut left_map), Value::Table(right_map)) => {
            if merge_depth > 0 {
                for (rname, rvalue) in right_map {
                    match left_map.remove(&rname) {
                        Some(lvalue) => {
                            let merged_value = merge_toml_values(lvalue, rvalue, merge_depth - 1);
                            left_map.insert(rname, merged_value);
                        }
                        None => {
                            left_map.insert(rname, rvalue);
                        }
                    }
                }
                Value::Table(left_map)
            } else {
                Value::Table(right_map)
            }
        }
        // Catch everything else we didn't handle, and use the right value
        (_, value) => value,
    }
}

/// Finds the current workspace folder.
/// Used as a ceiling dir for LSP root resolution, the filepicker and potentially as a future filewatching root
///
/// This function starts searching the FS upward from the CWD
/// and returns the first directory that contains either `.git`, `.svn`, `.jj` or `.helix`.
/// If no workspace was found returns (CWD, true).
/// Otherwise (workspace, false) is returned
pub fn find_workspace() -> (PathBuf, bool) {
    let current_dir = current_working_dir();
    find_workspace_in(current_dir)
}

pub fn find_workspace_in(dir: impl AsRef<Path>) -> (PathBuf, bool) {
    let dir = dir.as_ref();
    for ancestor in dir.ancestors() {
        if ancestor.join(".git").exists()
            || ancestor.join(".svn").exists()
            || ancestor.join(".jj").exists()
            || ancestor.join(".helix").exists()
        {
            return (ancestor.to_owned(), false);
        }
    }

    (dir.to_owned(), true)
}

fn default_config_file() -> PathBuf {
    config_dir().join("config.toml")
}

fn ensure_parent_dir(path: &Path) {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).ok();
        }
    }
}

#[cfg(test)]
mod merge_toml_tests {
    use std::str;

    use super::merge_toml_values;
    use toml::Value;

    #[test]
    fn language_toml_map_merges() {
        const USER: &str = r#"
        [[language]]
        name = "nix"
        test = "bbb"
        indent = { tab-width = 4, unit = "    ", test = "aaa" }
        "#;

        let base = include_bytes!("../languages.toml");
        let base = str::from_utf8(base).expect("Couldn't parse built-in languages config");
        let base: Value = toml::from_str(base).expect("Couldn't parse built-in languages config");
        let user: Value = toml::from_str(USER).unwrap();

        let merged = merge_toml_values(base, user, 3);
        let languages = merged.get("language").unwrap().as_array().unwrap();
        let nix = languages
            .iter()
            .find(|v| v.get("name").unwrap().as_str().unwrap() == "nix")
            .unwrap();
        let nix_indent = nix.get("indent").unwrap();

        // We changed tab-width and unit in indent so check them if they are the new values
        assert_eq!(
            nix_indent.get("tab-width").unwrap().as_integer().unwrap(),
            4
        );
        assert_eq!(nix_indent.get("unit").unwrap().as_str().unwrap(), "    ");
        // We added a new keys, so check them
        assert_eq!(nix.get("test").unwrap().as_str().unwrap(), "bbb");
        assert_eq!(nix_indent.get("test").unwrap().as_str().unwrap(), "aaa");
        // We didn't change comment-token so it should be same
        assert_eq!(nix.get("comment-token").unwrap().as_str().unwrap(), "#");
    }

    #[test]
    fn language_toml_nested_array_merges() {
        const USER: &str = r#"
        [[language]]
        name = "typescript"
        language-server = { command = "deno", args = ["lsp"] }
        "#;

        let base = include_bytes!("../languages.toml");
        let base = str::from_utf8(base).expect("Couldn't parse built-in languages config");
        let base: Value = toml::from_str(base).expect("Couldn't parse built-in languages config");
        let user: Value = toml::from_str(USER).unwrap();

        let merged = merge_toml_values(base, user, 3);
        let languages = merged.get("language").unwrap().as_array().unwrap();
        let ts = languages
            .iter()
            .find(|v| v.get("name").unwrap().as_str().unwrap() == "typescript")
            .unwrap();
        assert_eq!(
            ts.get("language-server")
                .unwrap()
                .get("args")
                .unwrap()
                .as_array()
                .unwrap(),
            &vec![Value::String("lsp".into())]
        )
    }
}

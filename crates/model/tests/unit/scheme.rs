use std::path::Path;

use model::color::{Base16, Palette, Rgb};
use model::config::Config;
use model::scheme::{self, Scheme, SchemeError};

const FULL: &str = r##"
base00 = "#000000"
base01 = "#010101"
base02 = "#020202"
base03 = "#030303"
base04 = "#040404"
base05 = "#050505"
base06 = "#060606"
base07 = "#070707"
base08 = "#080808"
base09 = "#090909"
base0A = "#0a0a0a"
base0B = "#0b0b0b"
base0C = "#0c0c0c"
base0D = "#0d0d0d"
base0E = "#0e0e0e"
base0F = "#0f0f0f"
"##;

fn named(name: &str) -> Scheme {
    Scheme::try_from(name.to_owned()).expect("valid scheme name")
}

fn write(dir: &Path, name: &str, text: &str) {
    std::fs::write(dir.join(format!("{name}.toml")), text).unwrap();
}

#[test]
fn the_default_scheme_needs_no_files() {
    let config: Config = toml::from_str("").unwrap();
    assert_eq!(config.theme.scheme, Scheme::Default);
    let palette = scheme::load(&Scheme::Default, Path::new("/nonexistent")).unwrap();
    assert_eq!(palette, Palette::DEFAULT);
}

#[test]
fn a_complete_scheme_stands_alone() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "mine", FULL);
    let palette = scheme::load(&named("mine"), dir.path()).unwrap();
    assert_eq!(palette[Base16::Base0A], Rgb::hex(0x0a0a0a));
}

#[test]
fn inherited_slots_come_from_the_parent_and_the_child_wins() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "base", FULL);
    write(
        dir.path(),
        "child",
        "inherits = \"base\"\nbase08 = \"#ff0000\"\n",
    );
    write(
        dir.path(),
        "tweak",
        "inherits = \"default\"\nbase00 = \"#004040\"\n",
    );
    let child = scheme::load(&named("child"), dir.path()).unwrap();
    assert_eq!(child[Base16::Base08], Rgb::hex(0xff0000));
    assert_eq!(child[Base16::Base07], Rgb::hex(0x070707));
    let tweak = scheme::load(&named("tweak"), dir.path()).unwrap();
    assert_eq!(tweak[Base16::Base00], Rgb::hex(0x004040));
    assert_eq!(tweak[Base16::Base0F], Palette::DEFAULT[Base16::Base0F]);
}

#[test]
fn a_scheme_without_inherits_names_its_missing_slots() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "half", "base00 = \"#000000\"\n");
    match scheme::load(&named("half"), dir.path()) {
        Err(SchemeError::Missing { slots, .. }) => {
            assert_eq!(slots.len(), 15);
            assert_eq!(slots[9].to_string(), "base0A");
        }
        other => panic!("expected missing slots, got {other:?}"),
    }
}

#[test]
fn inheriting_in_a_cycle_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a", "inherits = \"b\"\n");
    write(dir.path(), "b", "inherits = \"a\"\n");
    match scheme::load(&named("a"), dir.path()) {
        Err(SchemeError::Cycle { names }) => {
            let names: Vec<String> = names.iter().map(ToString::to_string).collect();
            assert_eq!(names, ["a", "b", "a"]);
        }
        other => panic!("expected a cycle, got {other:?}"),
    }
}

#[test]
fn unknown_keys_and_missing_files_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "typo",
        "inherits = \"default\"\nbase0g = \"#000000\"\n",
    );
    assert!(matches!(
        scheme::load(&named("typo"), dir.path()),
        Err(SchemeError::Parse { .. })
    ));
    assert!(matches!(
        scheme::load(&named("absent"), dir.path()),
        Err(SchemeError::Read { .. })
    ));
}

#[test]
fn scheme_names_are_one_plain_file_name() {
    for bad in ["", "../x", "dir/x", "x.toml"] {
        assert!(Scheme::try_from(bad.to_owned()).is_err(), "{bad:?}");
    }
    assert!(toml::from_str::<Config>("[theme]\nscheme = \"x.toml\"").is_err());
}

#[test]
fn the_example_scheme_is_the_built_in_default() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let palette = scheme::load(&named("default-scheme"), &root).expect("default-scheme.toml loads");
    assert_eq!(palette, Palette::DEFAULT);
}

use std::time::{Duration, SystemTime};

use state::insert::{KebabName, stamp};

#[test]
fn names_are_lowercase_words_joined_by_single_dashes() {
    assert_eq!(KebabName::of("My Photo (1)").as_str(), "my-photo-1");
    assert_eq!(
        KebabName::of("  Screen_Shot--2026  ").as_str(),
        "screen-shot-2026"
    );
    assert_eq!(KebabName::of("✓✓").as_str(), "image");
}

#[test]
fn stamps_are_utc_dates_and_times() {
    assert_eq!(stamp(SystemTime::UNIX_EPOCH), "1970-01-01-000000");
    let leap = SystemTime::UNIX_EPOCH + Duration::from_secs(951_868_800 - 1);
    assert_eq!(stamp(leap), "2000-02-29-235959");
    let later = SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_022_612);
    assert_eq!(stamp(later), "2026-10-26-135012");
}

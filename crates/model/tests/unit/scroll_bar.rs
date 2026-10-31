use model::scroll_bar::{OnTrack, Thumb};
use model::units::Px;

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn no_bar_shows_when_everything_fits() {
    assert_eq!(Thumb::of(0, 10, 0), None);
    let whole = Thumb::of(0, 10, 10).unwrap();
    assert_eq!(OnTrack::of(whole, Px(200.0)), None);
    let more = Thumb::of(0, 12, 10).unwrap();
    assert_eq!(OnTrack::of(more, Px(200.0)), None);
}

#[test]
fn the_thumb_spans_what_is_shown_and_stays_pinned_at_the_end() {
    let track = Px(200.0);
    let first = OnTrack::of(Thumb::of(0, 25, 100).unwrap(), track).unwrap();
    assert!(close(first.shown_top, 0.0) && close(first.shown_length, 0.25));
    let last = OnTrack::of(Thumb::of(75, 25, 100).unwrap(), track).unwrap();
    assert!(close(last.shown_top + last.shown_length, 1.0));
    let one = OnTrack::of(Thumb::of(99, 1, 100).unwrap(), track).unwrap();
    assert!(close(one.shown_top + one.shown_length, 1.0));
}

#[test]
fn small_thumbs_are_drawn_at_least_their_size_and_reach_both_ends() {
    let track = Px(200.0);
    let top = OnTrack::of(Thumb::of(0, 50, 100_000).unwrap(), track).unwrap();
    assert!(close(top.shown_length, 24.0 / 200.0) && close(top.shown_top, 0.0));
    let bottom = OnTrack::of(Thumb::of(99_950, 50, 100_000).unwrap(), track).unwrap();
    assert!(close(bottom.shown_top + bottom.shown_length, 1.0));
}

#[test]
fn a_press_grabs_the_thumb_where_it_lands_or_by_its_middle() {
    let on = OnTrack::of(Thumb::of(50, 10, 100).unwrap(), Px(400.0)).unwrap();
    assert!(close(on.grab(on.shown_top + 0.05), 0.05));
    assert!(close(on.grab(0.0), on.shown_length / 2.0));
    assert!(close(on.grab(1.0), on.shown_length / 2.0));
}

#[test]
fn dragging_inverts_the_geometry() {
    let (total, shown) = (1_000, 40);
    for first in [0, 1, 123, 480, 960] {
        let thumb = Thumb::of(first, shown, total).unwrap();
        let on = OnTrack::of(thumb, Px(400.0)).unwrap();
        assert!(close(on.top_at(on.shown_top), thumb.top));
    }
    let on = OnTrack::of(Thumb::of(0, shown, total).unwrap(), Px(400.0)).unwrap();
    assert!(close(on.top_at(-1.0), 0.0));
    assert!(close(on.top_at(2.0), 1.0 - on.thumb.length));
}

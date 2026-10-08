use model::command::Command;
use model::key::{Key, KeyCode, KeySeq};
use state::keymap::{Context, Keymap, Resolved};

#[test]
fn helix_tables_build() {
    Keymap::helix().expect("static tables have no conflicts");
}

#[test]
fn save_all_chord_in_every_editing_context() {
    let keymap = Keymap::helix().unwrap();
    for context in [Context::Normal, Context::Insert, Context::Select] {
        assert!(matches!(
            keymap.resolve(context, &[Key::ctrl('k')]),
            Resolved::Pending
        ));
        assert!(matches!(
            keymap.resolve(context, &[Key::ctrl('k'), Key::ctrl('s')]),
            Resolved::Command(Command::WriteAll)
        ));
    }
}

#[test]
fn unbound_after_prefix() {
    let keymap = Keymap::helix().unwrap();
    assert!(matches!(
        keymap.resolve(Context::Normal, &[Key::ctrl('k'), Key::char('x')]),
        Resolved::Unbound
    ));
    assert!(matches!(
        keymap.resolve(Context::Normal, &[Key::char('i'), Key::char('i')]),
        Resolved::Unbound
    ));
}

#[test]
fn key_seq_caps_at_max() {
    let full = (0..KeySeq::MAX).fold(KeySeq::EMPTY, |seq, _| seq.push(Key::char('g')).unwrap());
    assert_eq!(full.as_slice().len(), KeySeq::MAX);
    assert_eq!(full.push(Key::char('g')), None);
}

#[test]
fn key_display_uses_helix_notation() {
    assert_eq!(Key::ctrl('k').to_string(), "C-k");
    assert_eq!(Key::char(' ').to_string(), "space");
    assert_eq!(Key::code(KeyCode::Esc).to_string(), "esc");
}

#[test]
fn select_mode_shares_normal_bindings() {
    use model::command::Motion;
    let keymap = Keymap::helix().unwrap();
    for context in [Context::Normal, Context::Select] {
        assert!(matches!(
            keymap.resolve(context, &[Key::char('g'), Key::char('e')]),
            Resolved::Command(Command::Move(Motion::LastLine))
        ));
    }
    assert!(matches!(
        keymap.resolve(Context::Select, &[Key::char('v')]),
        Resolved::Command(Command::NormalMode)
    ));
    assert!(matches!(
        keymap.resolve(Context::Insert, &[Key::ctrl('w')]),
        Resolved::Command(Command::DeleteWordBackward)
    ));
}

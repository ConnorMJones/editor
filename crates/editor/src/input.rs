use gpui::Keystroke;
use model::key::{Key, KeyCode, Mods};

/// Maps a GPUI keystroke to the editor's key model; `None` for keys we don't bind.
pub fn translate(keystroke: &Keystroke) -> Option<Key> {
    let modifiers = &keystroke.modifiers;
    let mut mods = Mods::empty();
    mods.set(Mods::CTRL, modifiers.control);
    mods.set(Mods::ALT, modifiers.alt);
    let named = match keystroke.key.as_str() {
        "escape" => Some(KeyCode::Esc),
        "enter" => Some(KeyCode::Enter),
        "tab" => Some(KeyCode::Tab),
        "backspace" => Some(KeyCode::Backspace),
        "delete" => Some(KeyCode::Delete),
        "left" => Some(KeyCode::Left),
        "right" => Some(KeyCode::Right),
        "up" => Some(KeyCode::Up),
        "down" => Some(KeyCode::Down),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "pageup" => Some(KeyCode::PageUp),
        "pagedown" => Some(KeyCode::PageDown),
        "space" => Some(KeyCode::Char(' ')),
        key => key
            .strip_prefix('f')
            .and_then(|n| n.parse::<u8>().ok())
            .map(KeyCode::F),
    };
    match named {
        Some(KeyCode::Char(c)) => Some(Key {
            code: KeyCode::Char(c),
            mods,
        }),
        Some(code) => {
            mods.set(Mods::SHIFT, modifiers.shift);
            Some(Key { code, mods })
        }
        None => {
            let typed = match (mods.is_empty(), &keystroke.key_char) {
                (true, Some(typed)) => typed.as_str(),
                (true, None) | (false, _) => keystroke.key.as_str(),
            };
            let mut chars = typed.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Some(Key {
                    code: KeyCode::Char(match (modifiers.shift, mods.is_empty()) {
                        (true, false) => c.to_ascii_uppercase(),
                        (true, true) | (false, _) => c,
                    }),
                    mods,
                }),
                (None, _) | (Some(_), Some(_)) => None,
            }
        }
    }
}

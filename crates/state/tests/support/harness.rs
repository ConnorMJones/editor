// Shared by the unit and integration suites; each uses a subset.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use model::config::TabsConfig;
use model::key::{Key, KeyCode, Mods};
use model::register::Clipboard;
use state::buffer::{Buffer, BufferId, Content, Disk, TextDoc};
use state::layout::{PaneId, TextView, View};
use state::watch::{self, Watcher};
use state::{Effect, Root, State, input, job, register};

pub struct Harness {
    pub state: State,
    pub quit: bool,
    /// Stands in for the host clipboards: `[system, primary]`.
    pub clipboard: [Option<String>; 2],
    effects: Vec<Effect>,
    watcher: Option<(Watcher, mpsc::Receiver<Vec<PathBuf>>)>,
    _dir: Option<tempfile::TempDir>,
}

fn slot(clipboard: Clipboard) -> usize {
    match clipboard {
        Clipboard::System => 0,
        Clipboard::Primary => 1,
    }
}

impl Harness {
    fn boot(root: &Path, watcher: Option<(Watcher, mpsc::Receiver<Vec<PathBuf>>)>) -> Harness {
        let root = Root::open(root).expect("temp dir is a directory");
        let state = State::new(
            root,
            helix_core::config::default_lang_loader(),
            TabsConfig::default(),
        )
        .expect("keymap builds");
        let mut harness = Harness {
            state,
            quit: false,
            clipboard: [None, None],
            effects: Vec::new(),
            watcher,
            _dir: None,
        };
        state::boot(&mut harness.state, &mut harness.effects);
        harness.drain();
        harness
    }

    /// Runs a state operation that may queue effects, then drains them.
    pub fn run<R>(&mut self, f: impl FnOnce(&mut State, &mut Vec<Effect>) -> R) -> R {
        let result = f(&mut self.state, &mut self.effects);
        self.drain();
        result
    }

    /// Boots like the host does: index, root listing and git status run to completion.
    pub fn new(root: &Path) -> Harness {
        Harness::boot(root, None)
    }

    /// Like `new`, but with a live file watcher whose events `wait_for_events` delivers.
    pub fn watched(root: &Path) -> Harness {
        let (tx, rx) = mpsc::channel();
        let watcher = watch::start(move |paths| {
            let _ = tx.send(paths);
        })
        .expect("watcher starts");
        Harness::boot(root, Some((watcher, rx)))
    }

    /// Opens `name` in a fresh temp dir holding `marked` (helix `#[|]#` notation) and applies its selection.
    pub fn with(name: &str, marked: &str) -> Harness {
        let dir = tempfile::tempdir().expect("temp dir");
        let (text, selection) = helix_core::test::print(marked);
        std::fs::write(dir.path().join(name), text).expect("write fixture");
        let mut harness = Harness::new(dir.path());
        harness.keys(&format!(":o {name}<ret>"));
        harness.view_mut().selection = selection;
        harness._dir = Some(dir);
        harness
    }

    /// Types keys in helix notation: plain chars, `<esc>`, `<ret>`, `<C-x>`, `<A-x>`, `<S-tab>`.
    pub fn keys(&mut self, keys: &str) {
        let mut rest = keys;
        while let Some(c) = rest.chars().next() {
            let (key, len) = match (c, rest.find('>')) {
                ('<', Some(end)) if end > 1 => (special(&rest[1..end]), end + 1),
                _ => (Key::char(c), c.len_utf8()),
            };
            rest = &rest[len..];
            input::key(&mut self.state, key, &mut self.effects);
            self.drain();
        }
    }

    /// Feeds changed paths as the watcher would.
    pub fn changed(&mut self, paths: Vec<PathBuf>) {
        watch::changed(&mut self.state, paths, &mut self.effects);
        self.drain();
    }

    /// Delivers watcher batches until `done` holds or the timeout passes.
    pub fn wait_for_events(&mut self, done: impl Fn(&State) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match done(&self.state) {
                true => return true,
                false => {}
            }
            let batch = match &self.watcher {
                Some((_, rx)) => rx.recv_timeout(Duration::from_millis(50)),
                None => panic!("harness has no watcher"),
            };
            match batch {
                Ok(paths) => self.changed(paths),
                Err(_) => {}
            }
        }
        done(&self.state)
    }

    /// True when any watcher batch arrives within half a second.
    pub fn events_arrive(&mut self) -> bool {
        match &self.watcher {
            Some((_, rx)) => rx.recv_timeout(Duration::from_millis(500)).is_ok(),
            None => panic!("harness has no watcher"),
        }
    }

    /// Runs queued effects synchronously, as the host would.
    fn drain(&mut self) {
        while let Some(effect) = self.effects.pop() {
            match effect {
                Effect::Job(work) => {
                    let result = job::run(work);
                    job::complete(&mut self.state, result, &mut self.effects);
                }
                Effect::Quit => self.quit = true,
                Effect::Watch(dirs) => match &mut self.watcher {
                    Some((watcher, _)) => watch::watch(watcher, &dirs),
                    None => {}
                },
                Effect::WriteClipboard { clipboard, text } => {
                    self.clipboard[slot(clipboard)] = Some(text)
                }
                Effect::ReadClipboard { clipboard, then } => {
                    let text = self.clipboard[slot(clipboard)].clone();
                    register::clipboard_contents(&mut self.state, clipboard, then, text);
                }
            }
        }
    }

    /// Active view of the focused pane.
    pub fn view(&self) -> &TextView {
        match self.state.layout.panes[self.state.layout.focus].active() {
            Some(View::Text(view)) => view,
            None => panic!("focused pane has no tab"),
        }
    }

    pub fn view_mut(&mut self) -> &mut TextView {
        let focus = self.state.layout.focus;
        match self.state.layout.panes[focus].active_mut() {
            Some(View::Text(view)) => view,
            None => panic!("focused pane has no tab"),
        }
    }

    pub fn doc(&self) -> &TextDoc {
        match &self.state.buffers[self.view().buffer()].content {
            Content::Text { doc, .. } => doc,
            Content::Loading { .. } => panic!("focused buffer is still loading"),
        }
    }

    pub fn focused_path(&self) -> Option<PathBuf> {
        let pane = &self.state.layout.panes[self.state.layout.focus];
        pane.active()
            .map(|view| self.state.buffers[view.buffer()].path.clone())
    }

    /// File names of `pane`'s tabs in bar order, with the preview marked `*`.
    pub fn tabs(&self, pane: PaneId) -> Vec<String> {
        let target = &self.state.layout.panes[pane];
        let preview = target.tabs.as_ref().and_then(|tabs| tabs.preview());
        target
            .items()
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let path = &self.state.buffers[item.view.buffer()].path;
                let name = path.file_name().unwrap().to_string_lossy();
                match preview == Some(index) {
                    true => format!("{name}*"),
                    false => name.into_owned(),
                }
            })
            .collect()
    }

    /// Tabs of the focused pane; see `tabs`.
    pub fn focused_tabs(&self) -> Vec<String> {
        self.tabs(self.state.layout.focus)
    }

    /// The open buffer at `path` relative to the root.
    pub fn buffer(&self, name: &str) -> BufferId {
        let path = self.state.root.path().join(name);
        self.state
            .buffers
            .iter()
            .find(|(_, buffer)| buffer.path == path)
            .map(|(id, _)| id)
            .unwrap_or_else(|| panic!("{name} is not open"))
    }

    /// Text and selection in helix test notation.
    pub fn plain(&self) -> String {
        helix_core::test::plain(self.doc().text.clone(), &self.view().selection)
    }
}

/// Asserts that `keys` turn `before` into `after` (both in helix notation) in a `.txt` buffer.
#[track_caller]
pub fn check(before: &str, keys: &str, after: &str) {
    check_in("test.txt", before, keys, after)
}

/// Like helix's test harness, a trailing newline is added to both texts when missing.
#[track_caller]
pub fn check_in(name: &str, before: &str, keys: &str, after: &str) {
    // Markers are zero-width, so appending after them appends to the text.
    let terminate = |marked: &str| match helix_core::test::print(marked).0.ends_with('\n') {
        true => marked.to_owned(),
        false => format!("{marked}\n"),
    };
    let mut harness = Harness::with(name, &terminate(before));
    harness.keys(keys);
    let (text, selection) = helix_core::test::print(&terminate(after));
    assert_eq!(
        harness.plain(),
        helix_core::test::plain(text, &selection),
        "keys {keys:?} on {before:?}"
    );
}

fn special(name: &str) -> Key {
    let (mods, base) = match name.split_at_checked(2) {
        Some(("C-", base)) => (Mods::CTRL, base),
        Some(("A-", base)) => (Mods::ALT, base),
        Some(("S-", base)) => (Mods::SHIFT, base),
        _ => (Mods::empty(), name),
    };
    let code = match base {
        "esc" => KeyCode::Esc,
        "ret" => KeyCode::Enter,
        "bs" | "backspace" => KeyCode::Backspace,
        "del" => KeyCode::Delete,
        "tab" => KeyCode::Tab,
        "space" => KeyCode::Char(' '),
        "minus" => KeyCode::Char('-'),
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        single if single.chars().count() == 1 => {
            KeyCode::Char(single.chars().next().expect("one char"))
        }
        other => panic!("unknown key <{other}>"),
    };
    Key { code, mods }
}

pub fn disk_of(buffer: &Buffer) -> Disk {
    match &buffer.content {
        Content::Text { disk, .. } => *disk,
        Content::Loading { .. } => panic!("buffer is still loading"),
    }
}

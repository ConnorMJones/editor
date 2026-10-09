mod input;
mod preview;
mod prompt;
mod tabs;
mod text;
mod theme;
mod workspace;

use std::path::{Path, PathBuf};

use anyhow::{Context as _, anyhow};
use clap::Parser as _;
use gpui::{
    App, AppContext as _, TitlebarOptions, UniformListScrollHandle, WindowDecorations,
    WindowOptions, font, px, size,
};
use model::color::Palette;
use model::config::{Config, Restore};
use model::notice::Notice;
use state::workspace::{self as commands, Close};
use state::{Root, State, session, watch};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

use crate::theme::Theme;
use crate::workspace::{Drag, SessionFile, Workspace};

fn main() -> anyhow::Result<()> {
    let registry = tracing_subscriber::registry()
        .with(EnvFilter::try_from_env("EDITOR_LOG").unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer());
    #[cfg(feature = "tracy")]
    let registry = registry.with(tracing_tracy::TracyLayer::default());
    registry.init();

    let args = cli::Args::parse();
    let root = Root::open(&args.path)?;
    let config = match args.config {
        Some(path) => read_config(&path)?,
        None => match xdg_dir("XDG_CONFIG_HOME", ".config")?.join("editor/config.toml") {
            path if path.exists() => read_config(&path)?,
            _ => Config::default(),
        },
    };
    let palette_bytes = std::fs::read(&config.theme.palette)
        .with_context(|| format!("reading palette {:?}", config.theme.palette))?;
    let palette: Palette = serde_json::from_slice(&palette_bytes)
        .with_context(|| format!("parsing palette {:?}", config.theme.palette))?;
    let session_file = match config.session.restore {
        Restore::Off => SessionFile::Disabled,
        Restore::Last => SessionFile::At(session::file(
            &xdg_dir("XDG_STATE_HOME", ".local/state")?.join("editor/sessions"),
            root.path(),
        )),
    };
    let title = root.path().display().to_string();
    let state = State::new(root, helix_core::config::default_lang_loader(), config.tabs)?;
    let (changed_tx, changed_rx) = async_channel::unbounded::<Vec<PathBuf>>();
    let watcher = watch::start(move |paths| {
        // A closed channel means the window is gone; nothing left to notify.
        let _ = changed_tx.send_blocking(paths);
    })
    .context("starting file watcher")?;

    gpui_platform::application().run(move |cx: &mut App| {
        let installed = cx.text_system().all_font_names();
        let missing: Vec<&str> = [&config.font.mono, &config.font.ui]
            .into_iter()
            .map(|spec| spec.family.as_str())
            .filter(|family| !installed.iter().any(|name| name == family))
            .collect();
        match missing.as_slice() {
            [] => {}
            families => {
                tracing::error!(?families, "configured fonts are not installed");
                std::process::exit(1);
            }
        }
        let text_system = cx.text_system();
        let mono = text_system.resolve_font(&font(config.font.mono.family.clone()));
        let mono_size = px(config.font.mono.size_px);
        let cell = match text_system.advance(mono, mono_size, 'm') {
            Ok(advance) => size(
                advance.width,
                (text_system.ascent(mono, mono_size) + text_system.descent(mono, mono_size).abs())
                    .ceil(),
            ),
            Err(error) => {
                tracing::error!(%error, "cannot measure the monospace font");
                std::process::exit(1);
            }
        };
        let theme = Theme::new(&palette, &config, cell);
        let opened = cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(title.into()),
                    ..Default::default()
                }),
                app_id: Some("editor".to_owned()),
                window_decorations: Some(WindowDecorations::Client),
                window_min_size: Some(size(px(360.), px(240.))),
                ..Default::default()
            },
            |window, cx| {
                let workspace = cx.new(|cx| {
                    let focus = cx.focus_handle();
                    window.focus(&focus, cx);
                    Workspace {
                        state,
                        theme,
                        title_bar: config.window.title_bar,
                        effects: Vec::new(),
                        focus,
                        preedit: String::new(),
                        watcher,
                        tree_scroll: UniformListScrollHandle::new(),
                        tree_menu: None,
                        changes_scroll: UniformListScrollHandle::new(),
                        picker_scroll: UniformListScrollHandle::new(),
                        session: session_file,
                        drag: Drag::Idle,
                        resize_edge: None,
                        areas: Default::default(),
                        tab_scroll: Default::default(),
                        shown_tab: None,
                    }
                });
                workspace.update(cx, |workspace, cx| {
                    state::boot(&mut workspace.state, &mut workspace.effects);
                    match &workspace.session {
                        SessionFile::At(path) => {
                            let restored = session::load(path).and_then(|saved| match saved {
                                Some(saved) => session::restore(
                                    &mut workspace.state,
                                    saved,
                                    &mut workspace.effects,
                                ),
                                None => Ok(()),
                            });
                            match restored {
                                Ok(()) => {}
                                Err(error) => {
                                    workspace.state.notice = Some(Notice::SessionInvalid {
                                        path: path.clone(),
                                        reason: error.to_string(),
                                    })
                                }
                            }
                        }
                        SessionFile::Disabled => {}
                    }
                    workspace.flush(cx);
                    cx.spawn(async move |this, cx| {
                        while let Ok(mut paths) = changed_rx.recv().await {
                            cx.background_executor().timer(watch::DEBOUNCE).await;
                            while let Ok(more) = changed_rx.try_recv() {
                                paths.extend(more);
                            }
                            paths.sort_unstable();
                            paths.dedup();
                            match this.update(cx, |workspace, cx| workspace.on_paths(paths, cx)) {
                                Ok(()) => {}
                                Err(_) => break,
                            }
                        }
                    })
                    .detach();
                });
                // The compositor's close and Alt+F4 get `:q` semantics.
                window.on_window_should_close(cx, {
                    let workspace = workspace.downgrade();
                    move |window, cx| {
                        workspace
                            .update(cx, |workspace, cx| {
                                match commands::request_close(&mut workspace.state) {
                                    Close::Allowed => {
                                        workspace.save_session();
                                        true
                                    }
                                    Close::Refused => {
                                        window.focus(&workspace.focus, cx);
                                        cx.notify();
                                        false
                                    }
                                }
                            })
                            .unwrap_or(true)
                    }
                });
                workspace
            },
        );
        match opened {
            Ok(_) => cx.activate(true),
            Err(error) => {
                tracing::error!(%error, "failed to open window");
                std::process::exit(1);
            }
        }
    });
    Ok(())
}

fn read_config(path: &Path) -> anyhow::Result<Config> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading config {path:?}"))?;
    toml::from_str(&text).with_context(|| format!("parsing config {path:?}"))
}

/// `$var`, else `$HOME/fallback`.
fn xdg_dir(var: &str, fallback: &str) -> anyhow::Result<PathBuf> {
    match (std::env::var_os(var), std::env::var_os("HOME")) {
        (Some(dir), _) => Ok(PathBuf::from(dir)),
        (None, Some(home)) => Ok(PathBuf::from(home).join(fallback)),
        (None, None) => Err(anyhow!("neither {var} nor HOME is set")),
    }
}

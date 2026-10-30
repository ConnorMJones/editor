mod drawing;
mod fonts;
mod input;
mod page_tiles;
mod pictures;
mod preview;
mod prompt;
mod rendered;
mod sheet;
mod sheet_bar;
mod tabs;
mod text;
mod workspace;

use std::borrow::Cow;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context as _, anyhow};
use clap::Parser as _;
use cli::Attach;
use gpui::{
    App, AppContext as _, TitlebarOptions, UniformListScrollHandle, WindowDecorations,
    WindowOptions, font, px, size,
};
use model::command::Command;
use model::config::{Config, Restore};
use model::notice::Notice;
use model::scheme;
use model::theme::Theme;
use model::units::{Pt, Px};
use state::job::JobResult;
use state::render::typeset::{Look, Request};
use state::view::panes::{self as commands, Close};
use state::{Launch, State, project::session, project::watch};
use typeset::{FontSource, Typesetter};

use crate::fonts::Fonts;
use crate::workspace::{Drag, SessionFile, Workspace};

/// Reports every allocation to tracy with up to 16 frames of callstack.
#[cfg(feature = "tracy-alloc")]
#[global_allocator]
static ALLOCATOR: tracy_client::ProfiledAllocator<std::alloc::System> =
    tracy_client::ProfiledAllocator::new(std::alloc::System, 16);

fn main() -> anyhow::Result<()> {
    let args = cli::Args::parse();
    let attach = args.attach();
    #[cfg(any(debug_assertions, feature = "tracy"))]
    {
        use tracing_subscriber::EnvFilter;
        use tracing_subscriber::layer::SubscriberExt as _;
        use tracing_subscriber::util::SubscriberInitExt as _;
        let terminal = match attach {
            Attach::Relaunch | Attach::Logging => Some(tracing_subscriber::fmt::layer()),
            Attach::Background => None,
        };
        let registry = tracing_subscriber::registry()
            .with(EnvFilter::try_from_env("EDT_LOG").unwrap_or_else(|_| EnvFilter::new("info")))
            .with(terminal);
        #[cfg(feature = "tracy")]
        let registry = registry.with(match attach {
            Attach::Logging | Attach::Background => Some(tracing_tracy::TracyLayer::default()),
            Attach::Relaunch => None,
        });
        registry.init();
    }

    let (root, file) = match Launch::resolve(&args.path)? {
        Launch::Folder(root) => (root, None),
        Launch::File { root, file } => (root, Some(file)),
    };
    let (config, config_path) = match args.config {
        Some(path) => (read_config(&path)?, path),
        None => match xdg_dir("XDG_CONFIG_HOME", ".config")?.join("edt/config.toml") {
            path if path.exists() => (read_config(&path)?, path),
            path => (Config::default(), path),
        },
    };
    let palette = scheme::load(&config.theme.scheme, &config_path.with_file_name("schemes"))?;
    let session_file = match config.session.restore {
        Restore::Off => SessionFile::Disabled,
        Restore::Last => SessionFile::At(session::file(
            &xdg_dir("XDG_STATE_HOME", ".local/state")?.join("edt/sessions"),
            root.path(),
        )),
    };
    match attach {
        Attach::Relaunch => {
            std::process::Command::new(std::env::current_exe().context("finding edt's binary")?)
                .args(std::env::args_os().skip(1))
                .arg("--background")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .process_group(0)
                .spawn()
                .context("starting edt in the background")?;
            return Ok(());
        }
        Attach::Logging | Attach::Background => {}
    }
    let title = root.path().display().to_string();
    let prose = config.font.prose().family.clone();
    let (typeset_tx, typeset_rx) = std::sync::mpsc::channel::<Request>();
    let (typeset_done_tx, typeset_done_rx) = async_channel::unbounded::<JobResult>();
    // One thread owns the typesetter, so its fonts load once and its caches stay warm.
    std::thread::Builder::new()
        .name("typeset".to_owned())
        .spawn(move || {
            let mut typesetter = match Typesetter::new(FontSource::SystemAndEmbedded, &prose) {
                Ok(typesetter) => typesetter,
                Err(error) => {
                    tracing::error!(%error, "cannot start the typesetter");
                    std::process::exit(1);
                }
            };
            let mut text = String::new();
            while let Ok(request) = typeset_rx.recv() {
                let done = state::render::typeset::run(&mut typesetter, request, &mut text);
                match typeset_done_tx.send_blocking(JobResult::Typeset(done)) {
                    Ok(()) => {}
                    Err(_) => break,
                }
            }
        })
        .context("starting the typeset thread")?;
    let (changed_tx, changed_rx) = async_channel::unbounded::<Vec<PathBuf>>();
    let watcher = watch::start(move |paths| {
        // A closed channel means the window is gone; nothing left to notify.
        let _ = changed_tx.send_blocking(paths);
    })
    .context("starting file watcher")?;

    gpui_platform::application().run(move |cx: &mut App| {
        match cx
            .text_system()
            .add_fonts(model::fonts::BUNDLED.map(Cow::Borrowed).to_vec())
        {
            Ok(()) => {}
            Err(error) => {
                tracing::error!(%error, "cannot load the bundled fonts");
                std::process::exit(1);
            }
        }
        let installed = cx.text_system().all_font_names();
        let missing: Vec<&str> = [&config.font.mono, &config.font.ui, config.font.prose()]
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
        let prose = text_system.resolve_font(&font(config.font.prose().family.clone()));
        let prose_extent = [
            text_system.ascent(prose, px(1.)) / px(1.),
            text_system.descent(prose, px(1.)).abs() / px(1.),
        ];
        let theme = Theme::new(&palette, &config.theme);
        let fonts = Fonts::new(&config.font, cell, prose_extent);
        let look = Look {
            text: theme.text.rgb,
            prose_size: Pt::from(Px(fonts.prose_size / px(1.))),
            palette,
        };
        let state = match State::new(
            root,
            helix_core::config::default_lang_loader(),
            config.tabs,
            look,
        ) {
            Ok(state) => state,
            Err(error) => {
                tracing::error!(%error, "cannot build the editor state");
                std::process::exit(1);
            }
        };
        let opened = cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(title.into()),
                    ..Default::default()
                }),
                app_id: Some("edt".to_owned()),
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
                        fonts,
                        window: config.window,
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
                        drop_zone: None,
                        resize_edge: None,
                        areas: Default::default(),
                        tab_scroll: Default::default(),
                        shown_tab: None,
                        rendered: Default::default(),
                        typeset: typeset_tx,
                        pictures: Default::default(),
                        page_tiles: Default::default(),
                        window_height: window.viewport_size().height,
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
                    match file {
                        Some(file) => state::apply::apply(
                            &mut workspace.state,
                            Command::Open(file),
                            &mut workspace.effects,
                        ),
                        None => {}
                    }
                    workspace.flush(cx);
                    cx.spawn(async move |this, cx| {
                        while let Ok(done) = typeset_done_rx.recv().await {
                            let mut batch = vec![done];
                            while let Ok(more) = typeset_done_rx.try_recv() {
                                batch.push(more);
                            }
                            match this.update(cx, |workspace, cx| workspace.on_jobs(batch, cx)) {
                                Ok(()) => {}
                                Err(_) => break,
                            }
                        }
                    })
                    .detach();
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

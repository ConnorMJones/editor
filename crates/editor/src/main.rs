mod input;
mod theme;
mod workspace;

use std::path::{Path, PathBuf};

use anyhow::{Context as _, anyhow};
use clap::Parser as _;
use gpui::{App, AppContext as _, TitlebarOptions, WindowOptions};
use model::color::Palette;
use model::config::Config;
use state::{Root, State};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

use crate::theme::Theme;
use crate::workspace::Workspace;

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
        None => match default_config_path()? {
            path if path.exists() => read_config(&path)?,
            _ => Config::default(),
        },
    };
    let palette_bytes = std::fs::read(&config.theme.palette)
        .with_context(|| format!("reading palette {:?}", config.theme.palette))?;
    let palette: Palette = serde_json::from_slice(&palette_bytes)
        .with_context(|| format!("parsing palette {:?}", config.theme.palette))?;
    let theme = Theme::new(&palette, &config);
    let title = root.path().display().to_string();
    let state = State::new(root, helix_core::config::default_lang_loader())?;

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
        let opened = cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(title.into()),
                    ..Default::default()
                }),
                app_id: Some("editor".to_owned()),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    let focus = cx.focus_handle();
                    window.focus(&focus, cx);
                    Workspace {
                        state,
                        theme,
                        effects: Vec::new(),
                        focus,
                    }
                })
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

fn default_config_path() -> anyhow::Result<PathBuf> {
    let base = match (
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
    ) {
        (Some(xdg), _) => PathBuf::from(xdg),
        (None, Some(home)) => PathBuf::from(home).join(".config"),
        (None, None) => return Err(anyhow!("neither XDG_CONFIG_HOME nor HOME is set")),
    };
    Ok(base.join("editor").join("config.toml"))
}

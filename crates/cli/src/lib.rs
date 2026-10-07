use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "editor", version, about)]
pub struct Args {
    /// Directory to open.
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Config file; defaults to $XDG_CONFIG_HOME/editor/config.toml.
    #[arg(long)]
    pub config: Option<PathBuf>,
}

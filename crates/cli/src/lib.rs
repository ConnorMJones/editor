use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "edt", version, about)]
pub struct Args {
    /// Folder or file to open.
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Config file; defaults to $XDG_CONFIG_HOME/edt/config.toml.
    #[arg(long)]
    pub config: Option<PathBuf>,
}

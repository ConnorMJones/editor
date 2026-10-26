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
    /// Stay attached to the terminal until the window closes.
    #[arg(long, short)]
    pub wait: bool,
    #[arg(long, hide = true, conflicts_with = "wait")]
    pub detached: bool,
}

/// How this process relates to the terminal it was started from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attach {
    Launch,
    Wait,
    Detached,
}

impl Args {
    pub fn attach(&self) -> Attach {
        match (self.wait, self.detached) {
            (false, false) => Attach::Launch,
            (true, _) => Attach::Wait,
            (false, true) => Attach::Detached,
        }
    }
}

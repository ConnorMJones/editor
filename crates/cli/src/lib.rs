use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "edt", version, about)]
pub struct Args {
    #[arg(default_value = ".", help = "Folder or file to open.")]
    pub path: PathBuf,
    #[arg(
        long,
        help = "Config file: defaults to $XDG_CONFIG_HOME/edt/config.toml."
    )]
    pub config: Option<PathBuf>,
    #[arg(long, short, help = "Output logs to the terminal here.")]
    pub log: bool,
    #[arg(long, hide = true, conflicts_with = "log")]
    pub background: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attach {
    Relaunch,
    Logging,
    Background,
}

impl Args {
    pub fn attach(&self) -> Attach {
        match (self.log, self.background) {
            (false, false) => Attach::Relaunch,
            (true, _) => Attach::Logging,
            (false, true) => Attach::Background,
        }
    }
}

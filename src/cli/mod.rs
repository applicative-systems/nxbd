use clap::{Parser, Subcommand};

use crate::libnxbd;

#[derive(Parser, Debug)]
#[command(name = "nxbd", about = "CLI tool to build and switch systems")]
pub struct Cli {
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Build {
        #[arg(value_parser = libnxbd::flakeref::parse_flake_reference)]
        systems: Vec<libnxbd::FlakeReference>,
    },
    SwitchRemote {
        #[arg(value_parser = libnxbd::flakeref::parse_flake_reference)]
        systems: Vec<libnxbd::FlakeReference>,
    },
    SwitchLocal {
        #[arg(value_parser = libnxbd::flakeref::parse_flake_reference)]
        system: Option<libnxbd::FlakeReference>,
        #[arg(long)]
        ignore_hostname: bool,
    },
    Check {
        #[arg(value_parser = libnxbd::flakeref::parse_flake_reference)]
        systems: Vec<libnxbd::FlakeReference>,
        #[arg(long)]
        save_ignore: bool,
        #[arg(long, default_value = ".nxbd-ignore.yaml")]
        ignore_file: String,
    },
    Checks,
}

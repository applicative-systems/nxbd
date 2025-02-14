use clap::{Parser, Subcommand};

use crate::libnxbd;

/// CLI tool to manage systems
#[derive(Parser, Debug)]
#[command(name = "nxbd", about = "CLI tool to build and switch systems")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Build systems
    Build {
        /// Systems to build
        #[arg(value_parser = libnxbd::flakeref::parse_flake_reference)]
        systems: Vec<libnxbd::FlakeReference>,
    },
    /// Switch remote systems
    SwitchRemote {
        /// Systems to switch
        #[arg(value_parser = libnxbd::flakeref::parse_flake_reference)]
        systems: Vec<libnxbd::FlakeReference>,
    },
    /// Switch systems
    SwitchLocal {
        /// System attribute to switch to
        #[arg(value_parser = libnxbd::flakeref::parse_flake_reference)]
        system: Option<libnxbd::FlakeReference>,
    },
    /// Show information about the current user and systems
    Info {
        /// Systems to inspect
        #[arg(value_parser = libnxbd::flakeref::parse_flake_reference)]
        systems: Vec<libnxbd::FlakeReference>,
        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,
    },
}

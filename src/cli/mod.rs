use clap::{Parser, Subcommand};

use crate::nixlib;

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
        #[arg(value_parser = nixlib::flakeref::parse_flake_reference)]
        systems: Vec<nixlib::FlakeReference>,
    },
    /// Switch remote systems
    SwitchRemote {
        /// Systems to switch
        #[arg(value_parser = nixlib::flakeref::parse_flake_reference)]
        systems: Vec<nixlib::FlakeReference>,
    },
    /// Switch systems
    SwitchLocal {
        /// System attribute to switch to
        #[arg(value_parser = nixlib::flakeref::parse_flake_reference)]
        system: Option<nixlib::FlakeReference>,
    },
    /// Show information about the current user and systems
    Info {
        /// Systems to inspect
        #[arg(value_parser = nixlib::flakeref::parse_flake_reference)]
        systems: Vec<nixlib::FlakeReference>,
    },
}

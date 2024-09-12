use clap::{Parser, Subcommand};

use crate::nixlib;
use crate::nixlib::FlakeReference;

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
        systems: Vec<FlakeReference>,
    },
    /// Switch systems
    Switch {
        /// Systems to switch
        #[arg(value_parser = nixlib::flakeref::parse_flake_reference)]
        systems: Vec<FlakeReference>,
    },
}


use clap::{Parser, Subcommand};

use crate::libnxbd;

const SYSTEMS_HELP: &str = "System selection in flakes attribute syntax (e.g., `.#hostname` or `github:user/repo#hostname`).";
const SYSTEMS_ALL_HELP: &str = "Can be one or many. Will select all systems in the flake in the current directory if not specified.";

#[derive(Parser, Debug)]
#[command(name = "nxbd")]
#[command(about = "Build and deploy NixOS systems using flakes")]
#[command(
    long_about = "A tool for building and deploying NixOS systems using flakes. \
    It supports local and remote deployment, configuration checks, and automated system updates."
)]
pub struct Cli {
    #[arg(
        short,
        long,
        global = true,
        help = "Show detailed information during execution"
    )]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    #[command(about = "Build NixOS configurations without deploying")]
    #[command(
        long_about = "Build one or more NixOS system configurations without deploying them. \
        This is useful for testing builds or preparing systems for deployment."
    )]
    Build {
        #[arg(help = &format!("{} {}", SYSTEMS_HELP, SYSTEMS_ALL_HELP))]
        #[arg(value_parser = libnxbd::flakeref::parse_flake_reference)]
        systems: Vec<libnxbd::FlakeReference>,
    },

    #[command(about = "Deploy configurations to remote systems")]
    #[command(
        long_about = r#"Deploy NixOS configurations to one or more remote systems.
Supports configuration checks and automatic rebooting if needed.

This command assumes:

- Deployment target host address is defined by the hostname
and the (optional) FQDN and is obtained via `config.networking.fqdnOrHostName`.
- The local user account that runs `nxbd` is used for connecting to the target
  host via SSH."#
    )]
    SwitchRemote {
        #[arg(help = &format!("{} {}", SYSTEMS_HELP, SYSTEMS_ALL_HELP))]
        #[arg(value_parser = libnxbd::flakeref::parse_flake_reference)]
        systems: Vec<libnxbd::FlakeReference>,

        #[arg(long, help = "Skip pre-deployment configuration checks")]
        ignore_checks: bool,

        #[arg(
            long,
            help = "Automatically reboot if required by kernel/initrd changes"
        )]
        reboot: bool,
    },

    #[command(about = "Deploy configuration to the local system")]
    #[command(long_about = r#"Deploy a NixOS configuration to the local system.

If no system is specified, it uses the current hostname as the flake attribute selector in the flake of the current working directory."#)]
    SwitchLocal {
        #[arg(help = &format!("{} Defaults to `.#<hostname>` if not provided.", SYSTEMS_HELP))]
        #[arg(value_parser = libnxbd::flakeref::parse_flake_reference)]
        system: Option<libnxbd::FlakeReference>,

        #[arg(
            long,
            help = "Ignore hostname mismatch between system and configuration"
        )]
        ignore_hostname: bool,

        #[arg(long, help = "Skip pre-deployment configuration checks")]
        ignore_checks: bool,
    },

    #[command(about = "Run configuration checks")]
    #[command(long_about = "Run configuration checks on one or more systems. \
        Checks can verify system configuration, SSH keys, and other deployment requirements.")]
    Check {
        #[arg(help = &format!("{} {}", SYSTEMS_HELP, SYSTEMS_ALL_HELP))]
        #[arg(value_parser = libnxbd::flakeref::parse_flake_reference)]
        systems: Vec<libnxbd::FlakeReference>,

        #[arg(
            long,
            help = "Save any failing checks to the ignore file. They will be ignored in future runs."
        )]
        save_ignore: bool,

        #[arg(
            long,
            help = "Path to the ignore file",
            default_value = ".nxbd-ignore.yaml"
        )]
        ignore_file: String,
    },

    #[command(about = "List all available configuration checks")]
    #[command(
        long_about = "List all available configuration checks along with all their descriptions."
    )]
    Checks,

    #[command(about = "Show status of NixOS systems")]
    #[command(
        long_about = "Display detailed status information about one or more NixOS systems, \
        including deployment status, reboot requirements, and system health."
    )]
    Status {
        #[arg(help = &format!("{} {}", SYSTEMS_HELP, SYSTEMS_ALL_HELP))]
        #[arg(value_parser = libnxbd::flakeref::parse_flake_reference)]
        systems: Vec<libnxbd::FlakeReference>,
    },

    #[command(hide = true)]
    GenerateDocs {
        #[arg(help = "Directory where to generate the documentation")]
        output_dir: String,
    },
}

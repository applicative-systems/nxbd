mod cli;
mod nixlib;

use clap::Parser;
use eyre::Result;

use nixlib::FlakeReference;

use crate::cli::{Cli, Command};

fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    match &cli.command {
        Command::Build { systems } => {
            let system_attributes : Vec<FlakeReference> = if systems.is_empty() {
                nixlib::nixos_configuration_flakerefs(".")?
            } else {
                systems.clone()
            };
            println!("systems: {}", system_attributes.iter().map(|f| f.to_string()).collect::<Vec<String>>().join(" "));
        }
        Command::Switch { systems } => {
            if systems.is_empty() {
                println!("Switching to default system...");
            } else {
                println!("Switching to systems: {systems:?}");
            }
        }
    }

    /*
    println!("output: {:?}", nixlib::nixos_configuration_attributes("."));
    println!("output: {:?}", nixlib::nixos_fqdn(&FlakeReference{ flake_path: ".".to_string(), attribute: "marketing".to_string() }));
    println!("output: {:?}", nixlib::toplevel_output_path(&FlakeReference{ flake_path: ".".to_string(), attribute: "marketing".to_string() }));
 */

    Ok(())
}

mod cli;
mod nixlib;

use clap::Parser;
use nixlib::{deployinfo::{nixos_deploy_info, ConfigInfo}, FlakeReference};
use crate::cli::{Cli, Command};
use nix::unistd;

fn flakerefs_or_default(refs: &[FlakeReference]) -> Result<Vec<FlakeReference>, nixlib::NixError> {
    if refs.is_empty() {
        nixlib::nixos_configuration_flakerefs(".")
    } else {
        Ok(refs.to_owned())
    }
}

fn deploy_remote(system_attribute: &FlakeReference, host: &str) -> Result<(), nixlib::NixError> {
    let toplevel = nixlib::toplevel_output_path(system_attribute)?;
    println!("Built store path for {}: [{}]", system_attribute, toplevel);
    nixlib::copy_to_host(&toplevel, host)?;
    nixlib::activate_profile(&toplevel, true, Some(host))?;
    nixlib::switch_to_configuration(&toplevel, "switch", true, Some(host))
}

fn main() -> Result<(), nixlib::NixError> {
    let cli = Cli::parse();

    match &cli.command {
        Command::Build { systems } => {
            let system_attributes = flakerefs_or_default(systems)?;
            println!("Building systems: {}", system_attributes.iter().map(|f| 
                f.to_string()).collect::<Vec<String>>().join(" "));
            for system in &system_attributes {
                let toplevel = nixlib::toplevel_output_path(system)?;
                println!("Built store path for {}: [{}]", system, toplevel);
            }
        }
        Command::SwitchRemote { systems } => {
            let system_attributes = flakerefs_or_default(systems)?;
            println!("Switching systems: {}", system_attributes.iter().map(|f| f.to_string()).collect::<Vec<String>>().join(" "));

            let deploy_infos: Result<Vec<_>, _> = system_attributes
                .iter()
                .map(|sa| {
                    let deploy_info: ConfigInfo = nixos_deploy_info(sa)?;
                    match deploy_info.fqdn_or_host_name {
                        Some(host) => deploy_remote(sa, &host),
                        None => Err(nixlib::NixError::NoHostName)
                    }
                })
                .collect();
            println!("Infos: {deploy_infos:?}");
        }
        Command::SwitchLocal { system } => {
            let system_attribute = match system {
                Some(s) => s,
                None => {
                    let hostname = unistd::gethostname()
                        .expect("Failed getting hostname")
                        .into_string()
                        .expect("Hostname is no valid UTF-8");
                    &FlakeReference { 
                        url: ".".to_string(),
                        attribute: hostname
                    }
                }
            };
            println!("Switching system: {system_attribute}");

            let toplevel = nixlib::toplevel_output_path(system_attribute)?;
            println!("Store path is [{toplevel}]");
            nixlib::activate_profile(&toplevel, true, None)?;
            nixlib::switch_to_configuration(&toplevel, "switch", true, None)?;
        }
    }

    /*
    println!("output: {:?}", nixlib::nixos_configuration_attributes("."));
    println!("output: {:?}", nixlib::nixos_fqdn(&FlakeReference{ flake_path: ".".to_string(), attribute: "marketing".to_string() }));
    println!("output: {:?}", nixlib::toplevel_output_path(&FlakeReference{ flake_path: ".".to_string(), attribute: "marketing".to_string() }));
 */

    Ok(())
}

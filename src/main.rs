mod cli;
mod nixlib;

use crate::cli::{Cli, Command};
use clap::Parser;
use nix::unistd;
use nixlib::{
    configcheck::{get_standard_checks, CheckError},
    deployinfo::{nixos_deploy_info, ConfigInfo},
    userinfo::UserInfo,
    FlakeReference, NixError,
};
use owo_colors::OwoColorize;

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
            println!(
                "Building systems: {}",
                system_attributes
                    .iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<String>>()
                    .join(" ")
            );
            for system in &system_attributes {
                let toplevel = nixlib::toplevel_output_path(system)?;
                println!("Built store path for {}: [{}]", system, toplevel);
            }
        }
        Command::SwitchRemote { systems } => {
            let system_attributes = flakerefs_or_default(systems)?;
            println!(
                "Switching systems: {}",
                system_attributes
                    .iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<String>>()
                    .join(" ")
            );

            let deploy_infos: Result<Vec<_>, _> = system_attributes
                .iter()
                .map(|sa| {
                    let deploy_info: ConfigInfo = nixos_deploy_info(sa)?;
                    match deploy_info.fqdn_or_host_name {
                        Some(host) => deploy_remote(sa, &host),
                        None => Err(nixlib::NixError::NoHostName),
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
                        attribute: hostname,
                    }
                }
            };
            println!("Switching system: {system_attribute}");

            let toplevel = nixlib::toplevel_output_path(system_attribute)?;
            println!("Store path is [{toplevel}]");
            nixlib::activate_profile(&toplevel, true, None)?;
            nixlib::switch_to_configuration(&toplevel, "switch", true, None)?;
        }
        Command::Info { systems, verbose } => {
            let agent_info = UserInfo::collect();

            if *verbose {
                println!("Current user: {}", agent_info.username);
                println!("\nLoaded SSH keys in agent:");
                if agent_info.ssh_keys.is_empty() {
                    println!("  No SSH keys loaded in ssh-agent");
                } else {
                    for key in &agent_info.ssh_keys {
                        println!("Key:     {}", key);
                    }
                }
            }

            let system_attributes = flakerefs_or_default(systems)?;
            if *verbose {
                println!("\nSystem Configurations:");
            }

            for system in &system_attributes {
                println!("\n=== {} ===", system);
                match nixos_deploy_info(system) {
                    Ok(info) => {
                        if *verbose {
                            let hostname = info
                                .fqdn_or_host_name
                                .clone()
                                .unwrap_or_else(|| "unknown".to_string());
                            println!("Hostname: {}", hostname);
                            println!(
                                "SSH Service: {}",
                                if info.ssh_enabled {
                                    "enabled"
                                } else {
                                    "disabled"
                                }
                            );
                            println!(
                                "Wheel group sudo: {}",
                                if info.wheel_needs_password {
                                    "requires password"
                                } else {
                                    "passwordless"
                                }
                            );

                            println!("\nUsers with SSH access:");
                            for user in &info.users {
                                if !user.ssh_keys.is_empty() {
                                    println!(
                                        "\n  User: {} {}",
                                        user.name,
                                        if user.extra_groups.contains(&"wheel".to_string()) {
                                            "(wheel)"
                                        } else {
                                            ""
                                        }
                                    );
                                    println!("  Authorized keys:");
                                    for key in &user.ssh_keys {
                                        println!("    {}", key);
                                    }
                                }
                            }
                        }

                        println!("\nConfiguration Checks:");
                        for check in get_standard_checks() {
                            print!("  {} ... ", check.name);
                            match check.check(&info, &agent_info) {
                                Ok(()) => println!("{}", "✓".green()),
                                Err(errors) => {
                                    println!("{}", "✗".red());
                                    for error in errors {
                                        println!("    - {}", error);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => match e {
                        NixError::Eval(msg) => println!("Error evaluating system info:\n{}", msg),
                        _ => println!("Error getting system info: {:?}", e),
                    },
                }
            }
        }
    }

    Ok(())
}

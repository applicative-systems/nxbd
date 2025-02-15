mod cli;
mod libnxbd;

use crate::cli::{Cli, Command};
use clap::Parser;
use libnxbd::{
    configcheck::get_standard_checks,
    nixcommands::{
        activate_profile, copy_to_host, get_remote_builders, get_system,
        nixos_configuration_flakerefs, switch_to_configuration, toplevel_output_path,
    },
    nixosattributes::nixos_deploy_info,
    userinfo::UserInfo,
    FlakeReference, NixError,
};
use nix::unistd;
use owo_colors::OwoColorize;

// Add a constant for the arrow prefix
const ARROW: &str = "→";

fn flakerefs_or_default(refs: &[FlakeReference]) -> Result<Vec<FlakeReference>, libnxbd::NixError> {
    if refs.is_empty() {
        nixos_configuration_flakerefs(".")
    } else {
        Ok(refs.to_owned())
    }
}

fn deploy_remote(system_attribute: &FlakeReference, host: &str) -> Result<(), libnxbd::NixError> {
    let toplevel = toplevel_output_path(system_attribute)?;
    copy_to_host(&toplevel, host)?;
    activate_profile(&toplevel, true, Some(host))?;
    switch_to_configuration(&toplevel, "switch", true, Some(host))
}

fn main() -> Result<(), libnxbd::NixError> {
    let cli = Cli::parse();

    let user_info = UserInfo::collect()?;

    match &cli.command {
        Command::Build { systems } => {
            let system_attributes = flakerefs_or_default(systems)?;
            if system_attributes.len() > 1 {
                println!(
                    "{}",
                    format!(
                        "{} Building systems: {}",
                        ARROW,
                        system_attributes
                            .iter()
                            .map(|f| f.to_string())
                            .collect::<Vec<String>>()
                            .join(" ")
                    )
                    .white()
                );
            }
            for system in &system_attributes {
                let result = nixos_deploy_info(system)?;
                let remote_build = result.system != user_info.system
                    && !user_info
                        .remote_builders
                        .iter()
                        .any(|rb| rb.system == result.system);
                println!(
                    "{}",
                    format!("{} Building system: {}", ARROW, system).white()
                );
                let toplevel = toplevel_output_path(system)?;
                println!(
                    "{}",
                    format!("{} Built store path for {}: {}", ARROW, system, toplevel).white()
                );
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

            let deploy_infos: Vec<(FlakeReference, Result<(), libnxbd::NixError>)> =
                system_attributes
                    .iter()
                    .map(|sa| {
                        let result =
                            nixos_deploy_info(sa).and_then(|deploy_info| {
                                match deploy_info.fqdn_or_host_name {
                                    Some(host) => deploy_remote(sa, &host),
                                    None => Err(libnxbd::NixError::NoHostName),
                                }
                            });
                        (sa.clone(), result)
                    })
                    .collect();

            println!("\nDeployment Summary:");
            for (system, result) in deploy_infos {
                match result {
                    Ok(()) => println!("  {} {}", "✓".green(), system),
                    Err(e) => println!("  {} {} ({})", "✗".red(), system, e),
                }
            }
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

            let toplevel = toplevel_output_path(system_attribute)?;
            println!("Store path is [{toplevel}]");
            activate_profile(&toplevel, true, None)?;
            switch_to_configuration(&toplevel, "switch", true, None)?;
        }
        Command::Check { systems, verbose } => {
            if *verbose {
                println!("Current user: {}", user_info.username);
                println!("\nLoaded SSH keys in agent:");
                if user_info.ssh_keys.is_empty() {
                    println!("  No SSH keys loaded in ssh-agent");
                } else {
                    for key in &user_info.ssh_keys {
                        println!("Key:     {}", key);
                    }
                }
            }

            let system_attributes = flakerefs_or_default(systems)?;
            if *verbose {
                println!("\nSystem Configurations:");
            }

            for system in &system_attributes {
                println!("\n=== {} ===", system.to_string().cyan().bold());
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
                            match check.check(&info, &user_info) {
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
        Command::Checks => {
            println!("Available configuration checks:\n");
            for check in get_standard_checks() {
                println!("{}", check.name.cyan().bold());
                println!("  {}\n", check.description);
            }
        }
    }

    Ok(())
}

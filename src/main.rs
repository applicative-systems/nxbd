mod cli;
mod libnxbd;

use crate::cli::{Cli, Command};
use clap::Parser;
use libnxbd::{
    configcheck::{get_standard_checks, run_all_checks, save_failed_checks_to_ignore_file},
    nixcommands::{
        activate_profile, copy_to_host, nixos_configuration_flakerefs, realise_drv_remotely,
        realise_toplevel_output_path, switch_to_configuration,
    },
    nixosattributes::nixos_deploy_info,
    userinfo::UserInfo,
    FlakeReference, NixError,
};
use nix::unistd;
use owo_colors::OwoColorize;

fn passed_symbol(passed: bool) -> String {
    if passed {
        "✓".green().to_string()
    } else {
        "✗".red().to_string()
    }
}

fn flakerefs_or_default(refs: &[FlakeReference]) -> Result<Vec<FlakeReference>, libnxbd::NixError> {
    if refs.is_empty() {
        nixos_configuration_flakerefs(".")
    } else {
        Ok(refs.to_owned())
    }
}

fn deploy_remote(
    system_attribute: &FlakeReference,
    toplevel_out: &str,
    toplevel_drv: &str,
    host: &str,
    build_remote: bool,
) -> Result<(), libnxbd::NixError> {
    if build_remote {
        println!("{}", format!("→ Building on remote host: {}", host).white());
        copy_to_host(&toplevel_drv, host)?;
        println!("lol");
        realise_drv_remotely(&toplevel_drv, host)?;
    } else {
        let outpath = realise_toplevel_output_path(system_attribute)?;
        // We should change this in a way that realise_toplevel_output_path actually accepts the .drv file, but that may be impeding to nix-output-monitor
        assert_eq!(
            outpath, toplevel_out,
            "Built output path does not match evaluated output path"
        );
        copy_to_host(&toplevel_out, host)?;
    }

    activate_profile(&toplevel_out, true, Some(host))?;
    switch_to_configuration(&toplevel_out, "switch", true, Some(host))
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
                        "→ Building systems: {}",
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
                println!("{}", format!("→ Building system: {}", system).white());
                println!(
                    "{}",
                    format!("→ Built store path for {}: {}", system, result.toplevel_out).white()
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
                                    Some(host) => deploy_remote(
                                        sa,
                                        &deploy_info.toplevel_out,
                                        &deploy_info.toplevel_drv,
                                        &host,
                                        !user_info.can_build_natively(&deploy_info.system),
                                    ),
                                    _ => Err(libnxbd::NixError::NoHostName),
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
                _ => {
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

            let deploy_info = nixos_deploy_info(&system_attribute)?;
            let toplevel = deploy_info.toplevel_out;
            println!("Store path is [{toplevel}]");
            activate_profile(&toplevel, true, None)?;
            switch_to_configuration(&toplevel, "switch", true, None)?;
        }
        Command::Check {
            systems,
            verbose,
            save_ignore,
        } => {
            let system_attributes = flakerefs_or_default(systems)?;
            println!("\nSystem Configurations:");

            let mut all_results = Vec::new();

            for system in &system_attributes {
                println!("\n=== {} ===", system.to_string().cyan().bold());
                match nixos_deploy_info(system) {
                    Ok(info) => {
                        let results = run_all_checks(&info, &user_info);
                        let results_for_display = results.clone();

                        for (group_id, group_passed, check_results) in results_for_display {
                            // Find and display group information
                            if let Some(group) =
                                get_standard_checks().into_iter().find(|g| g.id == group_id)
                            {
                                println!(
                                    "{} - {}: {}\n{}\n",
                                    group.id.cyan().bold(),
                                    group.name.bold(),
                                    passed_symbol(group_passed),
                                    group.description.dimmed()
                                );
                            }
                            for (check_id, check_passed) in check_results {
                                println!("  {}: {}", check_id, passed_symbol(check_passed));
                                if *verbose && !check_passed {
                                    // Find the check details from standard checks
                                    if let Some(check) = get_standard_checks()
                                        .into_iter()
                                        .find(|g| g.id == group_id)
                                        .and_then(|g| {
                                            g.checks.into_iter().find(|c| c.id == check_id)
                                        })
                                    {
                                        println!(
                                            "    - {}\n      {}\n",
                                            check.description,
                                            check.advice.dimmed()
                                        );
                                    }
                                }
                            }
                        }

                        all_results.push((system, results));
                    }
                    Err(e) => match e {
                        NixError::Eval(msg) => println!("Error evaluating system info:\n{}", msg),
                        _ => println!("Error getting system info: {:?}", e),
                    },
                }
            }

            if *save_ignore {
                if let Err(e) = save_failed_checks_to_ignore_file(&all_results) {
                    eprintln!("Failed to save ignore file: {}", e);
                } else {
                    println!("Created .nxbd-ignore.yaml with failed checks");
                }
            }
        }
        Command::Checks => {
            println!("Available configuration checks:\n");
            for group in get_standard_checks() {
                println!(
                    "\n{} - {}\n{}\n",
                    group.id.cyan().bold(),
                    group.name.bold(),
                    group.description.dimmed()
                );

                for check in group.checks {
                    println!(
                        "  {} - {}\n    {}\n",
                        check.id.yellow(),
                        check.description,
                        check.advice.dimmed()
                    );
                }
            }
        }
    }

    Ok(())
}

mod cli;
mod libnxbd;

use crate::cli::{Cli, Command};
use clap::Parser;
use libnxbd::{
    configcheck::{
        get_standard_checks, load_ignored_checks, run_all_checks,
        save_failed_checks_to_ignore_file, CheckGroupResult,
    },
    nixcommands::{
        activate_profile, check_system_status, copy_to_host, nixos_configuration_flakerefs,
        realise_drv_remotely, realise_toplevel_output_paths, reboot_host, switch_to_configuration,
        SystemStatus,
    },
    nixosattributes::{nixos_deploy_info, ConfigInfo},
    userinfo::UserInfo,
    FlakeReference, NixError,
};
use nix::unistd;
use owo_colors::OwoColorize;
use rayon::prelude::*;
use std::fmt;
use std::fs::{self, create_dir_all};
use std::io;

#[derive(Debug)]
enum NxbdError {
    ChecksFailed {
        failures: Vec<(FlakeReference, Vec<(String, String)>)>, // (system, [(group_id, check_id)])
        is_switch: bool,
    },
    LocalHostnameMismatch {
        config_hostname: String,
        local_hostname: String,
    },
    Nix(NixError),
    Io(io::Error),
}

impl fmt::Display for NxbdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ChecksFailed {
                failures,
                is_switch,
            } => {
                writeln!(f, "The following checks failed:")?;
                for (system, checks) in failures {
                    writeln!(f, "\nSystem {}:", system)?;
                    for (group, check) in checks {
                        writeln!(f, "  - {}.{}", group, check)?;
                    }
                }
                writeln!(f)?;
                if *is_switch {
                    write!(f, "To proceed, either:\n - Fix the failing checks\n - Run 'nxbd check --save-ignore' to ignore these checks\n - Rerun with --ignore-checks")
                } else {
                    write!(f, "To proceed, either:\n - Fix the failing checks\n - Run 'nxbd check --save-ignore' to ignore these checks")
                }
            }
            Self::LocalHostnameMismatch {
                config_hostname,
                local_hostname,
            } => {
                write!(f, "Hostname mismatch: system config has '{}' but local system is '{}'\nTo proceed, either:\n - Fix the hostname\n - Rerun with --ignore-hostname", 
                    config_hostname, local_hostname)
            }
            Self::Nix(e) => write!(f, "{}", e),
            Self::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for NxbdError {}

impl From<NixError> for NxbdError {
    fn from(err: NixError) -> Self {
        NxbdError::Nix(err)
    }
}

impl From<io::Error> for NxbdError {
    fn from(err: io::Error) -> Self {
        NxbdError::Io(err)
    }
}

fn passed_symbol(passed: bool) -> String {
    if passed {
        "✅".green().to_string()
    } else {
        "❌".red().to_string()
    }
}

fn passed_ignore_symbol(passed: bool, ignored: bool) -> String {
    if !passed && ignored {
        "🙈".to_string()
    } else {
        passed_symbol(passed)
    }
}

fn flakerefs_or_default(refs: &[FlakeReference]) -> Result<Vec<FlakeReference>, libnxbd::NixError> {
    if refs.is_empty() {
        nixos_configuration_flakerefs(".")
    } else {
        Ok(refs.to_owned())
    }
}

fn run_system_checks(
    system: &FlakeReference,
    info: &ConfigInfo,
    user_info: &UserInfo,
    ignore_file: &str,
) -> Result<Vec<(String, String)>, NixError> {
    let ignored_checks = load_ignored_checks(ignore_file);
    let results = run_all_checks(info, user_info, ignored_checks.as_ref(), system);
    let mut failures = Vec::new();

    for group in &results {
        for check in &group.checks {
            if !check.passed && !check.ignored {
                failures.push((group.id.clone(), check.id.clone()));
            }
        }
    }

    Ok(failures)
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), NxbdError> {
    let cli = Cli::parse();
    let user_info = UserInfo::collect()?;

    if cli.verbose {
        println!("\nLocal Deployment Configuration:");
        println!("  User: {}", user_info.username.cyan());

        if !user_info.ssh_keys.is_empty() {
            println!("\n  SSH Keys:");
            for key in &user_info.ssh_keys {
                println!("    - {}", key.dimmed());
            }
        }

        let mut build_platforms = vec![user_info.system.clone()];
        build_platforms.extend(user_info.extra_platforms.clone());

        println!("\n  Build Capabilities:");
        println!("    Local: {}", build_platforms.join(", ").cyan());

        if !user_info.remote_builders.is_empty() {
            let remote_systems: Vec<_> = user_info
                .remote_builders
                .iter()
                .map(|rb| format!("{} via {}", rb.system, rb.ssh_host))
                .collect();
            println!("    Remote: {}", remote_systems.join(", ").cyan());
        }
        println!();
    }

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
        Command::SwitchRemote {
            systems,
            ignore_checks,
            reboot,
        } => {
            let system_attributes = flakerefs_or_default(systems)?;

            eprintln!(
                "Reading configurations of {}...",
                system_attributes
                    .iter()
                    .map(|s| format!(".#{}", s.attribute))
                    .collect::<Vec<_>>()
                    .join(" ")
            );

            // Parallelize deploy info collection
            let deploy_infos: Vec<(FlakeReference, Result<ConfigInfo, NixError>)> =
                system_attributes
                    .par_iter()
                    .map(|system| (system.clone(), nixos_deploy_info(system)))
                    .collect();

            println!(
                "Switching systems: {}",
                deploy_infos
                    .iter()
                    .filter_map(|(_, info)| info.as_ref().ok())
                    .map(|info| info.fqdn_or_host_name.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            );

            // Run checks first (unless ignored)
            if !ignore_checks {
                let mut all_failures = Vec::new();
                for (system, info) in &deploy_infos {
                    match info {
                        Ok(info) => {
                            let failures =
                                run_system_checks(system, info, &user_info, ".nxbd-ignore.yaml")?;
                            if !failures.is_empty() {
                                all_failures.push((system.clone(), failures));
                            }
                        }
                        Err(e) => return Err(e.clone().into()),
                    }
                }

                if !all_failures.is_empty() {
                    return Err(NxbdError::ChecksFailed {
                        failures: all_failures,
                        is_switch: true,
                    });
                }
            }

            // Split systems into local and remote builds based on build capability
            let (local_builds, remote_builds): (Vec<_>, Vec<_>) = deploy_infos
                .iter()
                .filter_map(|(system, info_result)| {
                    info_result.as_ref().ok().map(|info| (system, info))
                })
                .partition(|(_, info)| user_info.can_build_natively(&info.system));

            // Deploy systems that can be built locally
            if !local_builds.is_empty() {
                let local_systems: Vec<FlakeReference> =
                    local_builds.iter().map(|(sa, _)| (*sa).clone()).collect();
                realise_toplevel_output_paths(&local_systems)?;
            }

            let local_results: Vec<(FlakeReference, Result<(), NixError>)> = local_builds
                .into_iter()
                .map(|(sa, deploy_info)| {
                    let result =
                        copy_to_host(&deploy_info.toplevel_out, &deploy_info.fqdn_or_host_name)
                            .and_then(|_| {
                                activate_profile(
                                    &deploy_info.toplevel_out,
                                    true,
                                    Some(&deploy_info.fqdn_or_host_name),
                                )
                            })
                            .and_then(|_| {
                                switch_to_configuration(
                                    &deploy_info.toplevel_out,
                                    "switch",
                                    true,
                                    Some(&deploy_info.fqdn_or_host_name),
                                )
                            });
                    (sa.clone(), result)
                })
                .collect();

            // Deploy systems that need remote building
            let remote_results: Vec<(FlakeReference, Result<(), NixError>)> = remote_builds
                .into_iter()
                .map(|(sa, deploy_info)| {
                    println!(
                        "{}",
                        format!(
                            "→ Building on remote host: {}",
                            deploy_info.fqdn_or_host_name
                        )
                        .white()
                    );
                    let result =
                        copy_to_host(&deploy_info.toplevel_drv, &deploy_info.fqdn_or_host_name)
                            .and_then(|_| {
                                realise_drv_remotely(
                                    &deploy_info.toplevel_drv,
                                    &deploy_info.fqdn_or_host_name,
                                )
                            })
                            .and_then(|_| {
                                activate_profile(
                                    &deploy_info.toplevel_out,
                                    true,
                                    Some(&deploy_info.fqdn_or_host_name),
                                )
                            })
                            .and_then(|_| {
                                switch_to_configuration(
                                    &deploy_info.toplevel_out,
                                    "switch",
                                    true,
                                    Some(&deploy_info.fqdn_or_host_name),
                                )
                            });
                    (sa.clone(), result)
                })
                .collect();

            // Combine results for summary
            let results: Vec<_> = local_results.into_iter().chain(remote_results).collect();

            println!("\nDeployment Summary:");
            for (system, result) in results {
                match result {
                    Ok(()) => {
                        let (status_suffix, do_reboot) = deploy_infos
                            .iter()
                            .find(|(s, _)| s == &system)
                            .and_then(|(_, i)| i.as_ref().ok())
                            .and_then(|info| {
                                check_system_status(Some(&info.fqdn_or_host_name)).ok()
                            })
                            .map_or(("", false), |sys_status| match sys_status {
                                SystemStatus::Reachable { needs_reboot, .. } => (
                                    if needs_reboot {
                                        " (reboot required)"
                                    } else {
                                        ""
                                    },
                                    needs_reboot,
                                ),
                                SystemStatus::Unreachable => ("", false),
                            });

                        println!("  {} {}{}", "✓".green(), system, status_suffix);

                        if do_reboot && *reboot {
                            if let Some(info) = deploy_infos
                                .iter()
                                .find(|(s, _)| s == &system)
                                .and_then(|(_, i)| i.as_ref().ok())
                            {
                                print!("    Rebooting... ");
                                match reboot_host(&info.fqdn_or_host_name) {
                                    Ok(()) => println!("initiated"),
                                    Err(e) => println!("failed: {}", e),
                                }
                            }
                        }
                    }
                    Err(e) => println!("  {} {} ({})", "✗".red(), system, e),
                }
            }
        }
        Command::SwitchLocal {
            system,
            ignore_hostname,
            ignore_checks,
        } => {
            let local_hostname = unistd::gethostname()
                .expect("Failed getting hostname")
                .into_string()
                .expect("Hostname is no valid UTF-8");

            let system_attribute = match system {
                Some(s) => s,
                _ => &FlakeReference {
                    url: ".".to_string(),
                    attribute: local_hostname.clone(),
                },
            };
            println!("Switching system: {system_attribute}");

            let deploy_info = nixos_deploy_info(&system_attribute)?;

            // Run checks first (unless ignored)
            if !ignore_checks {
                let failures = run_system_checks(
                    &system_attribute,
                    &deploy_info,
                    &user_info,
                    ".nxbd-ignore.yaml",
                )?;
                if !failures.is_empty() {
                    return Err(NxbdError::ChecksFailed {
                        failures: vec![(system_attribute.clone(), failures)],
                        is_switch: true,
                    });
                }
            }

            // Check hostname match unless ignored
            if !ignore_hostname {
                let config_hostname = &deploy_info.host_name;
                if config_hostname != &local_hostname {
                    return Err(NxbdError::LocalHostnameMismatch {
                        config_hostname: config_hostname.clone(),
                        local_hostname: local_hostname.clone(),
                    });
                }
            }

            let toplevel = deploy_info.toplevel_out.clone();
            realise_toplevel_output_paths(&[system_attribute.clone()])?;
            activate_profile(&toplevel, true, None)?;
            switch_to_configuration(&toplevel, "switch", true, None)?;

            match check_system_status(None)? {
                SystemStatus::Reachable { needs_reboot, .. } => {
                    if needs_reboot {
                        println!("System update complete. Reboot required.");
                    } else {
                        println!("System update complete.");
                    }
                }
                SystemStatus::Unreachable => {
                    println!("System update complete. Reboot status unknown.");
                }
            }
        }
        Command::Check {
            systems,
            save_ignore,
            ignore_file,
        } => {
            let system_attributes = flakerefs_or_default(systems)?;
            let ignored_checks = load_ignored_checks(&ignore_file);

            eprintln!(
                "Reading configurations of {}...",
                system_attributes
                    .iter()
                    .map(|s| format!(".#{}", s.attribute))
                    .collect::<Vec<_>>()
                    .join(" ")
            );

            let deploy_infos: Vec<(FlakeReference, Result<ConfigInfo, NixError>)> =
                system_attributes
                    .par_iter()
                    .map(|system| (system.clone(), nixos_deploy_info(system)))
                    .collect();

            // Check if any deploy infos failed to evaluate
            let failed_systems: Vec<_> = deploy_infos
                .iter()
                .filter_map(|(system, result)| result.as_ref().err().map(|e| (system, e)))
                .collect();

            if !failed_systems.is_empty() {
                eprintln!("\nFailed to evaluate the following systems:");
                for (system, error) in &failed_systems {
                    eprintln!("  {} - {}", system, error);
                }
                let first_error = failed_systems[0].1.clone();
                return Err(NixError::from(first_error).into());
            }

            let all_results: Vec<(&FlakeReference, Vec<CheckGroupResult>)> = deploy_infos
                .iter()
                .filter_map(|(system, info)| {
                    info.as_ref().ok().map(|i| {
                        (
                            system,
                            run_all_checks(i, &user_info, ignored_checks.as_ref(), system),
                        )
                    })
                })
                .collect();

            for (system, check_group_results) in &all_results {
                eprintln!("\n=== {} ===", system.to_string().cyan().bold());

                let all_passed_or_ignored = check_group_results.iter().all(|group| {
                    group
                        .checks
                        .iter()
                        .all(|check| check.passed || check.ignored)
                });

                if all_passed_or_ignored {
                    let total_checks: usize =
                        check_group_results.iter().map(|g| g.checks.len()).sum();
                    let total_ignored: usize = check_group_results
                        .iter()
                        .map(|g| g.checks.iter().filter(|c| c.ignored).count())
                        .sum();

                    eprintln!(
                        "{} {} checks passed ({} ignored fails)",
                        passed_symbol(true),
                        total_checks,
                        total_ignored
                    );

                    if !cli.verbose {
                        continue;
                    }
                }

                for group_result in check_group_results {
                    let no_unignored_failures = group_result
                        .checks
                        .iter()
                        .all(|check| check.passed || check.ignored);

                    if no_unignored_failures && !cli.verbose {
                        continue;
                    }

                    let checks_count = group_result.checks.len();
                    let passed_count = group_result
                        .checks
                        .iter()
                        .filter(|check| check.passed)
                        .count();
                    let ignored_count = group_result
                        .checks
                        .iter()
                        .filter(|check| check.ignored)
                        .count();

                    eprintln!(
                        "\n{} - {} ({} checks, {} passed, {} ignored)",
                        group_result.id.cyan().bold(),
                        group_result.name.bold(),
                        checks_count,
                        passed_count,
                        ignored_count
                    );
                    eprintln!("{}", group_result.description);
                    eprintln!();

                    for check_result in &group_result.checks {
                        eprintln!(
                            "  {} {} - {}",
                            passed_ignore_symbol(check_result.passed, check_result.ignored),
                            check_result.id.yellow(),
                            check_result.description
                        );
                        if !check_result.passed {
                            eprintln!("    - {}", check_result.advice.dimmed());
                        }
                    }
                }
            }

            println!();

            let had_failures = all_results.iter().any(|(_, results)| {
                results.iter().any(|group| {
                    group
                        .checks
                        .iter()
                        .any(|check| !check.passed && !check.ignored)
                })
            });

            if *save_ignore {
                if let Err(e) = save_failed_checks_to_ignore_file(&ignore_file, &all_results) {
                    eprintln!("Failed to save ignore file: {}", e);
                } else {
                    println!("Created {} with failed checks", ignore_file);
                }
            } else if had_failures {
                let failures: Vec<(FlakeReference, Vec<(String, String)>)> = all_results
                    .iter()
                    .filter_map(|(system, results)| {
                        let failures: Vec<(String, String)> = results
                            .iter()
                            .flat_map(|group| {
                                group
                                    .checks
                                    .iter()
                                    .filter(|check| !check.passed && !check.ignored)
                                    .map(|check| (group.id.clone(), check.id.clone()))
                            })
                            .collect();
                        if !failures.is_empty() {
                            Some(((*system).clone(), failures))
                        } else {
                            None
                        }
                    })
                    .collect();

                return Err(NxbdError::ChecksFailed {
                    failures,
                    is_switch: false,
                });
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
        Command::Status { systems } => {
            let system_attributes = flakerefs_or_default(systems)?;

            eprintln!(
                "Reading configurations of {}...",
                system_attributes
                    .iter()
                    .map(|s| format!(".#{}", s.attribute))
                    .collect::<Vec<_>>()
                    .join(" ")
            );

            let deploy_infos: Vec<(FlakeReference, Result<ConfigInfo, NixError>)> =
                system_attributes
                    .par_iter()
                    .map(|system| (system.clone(), nixos_deploy_info(system)))
                    .collect();

            println!(
                "Querying status of {}...",
                deploy_infos
                    .iter()
                    .filter_map(|(_, info)| info.as_ref().ok())
                    .map(|info| info.fqdn_or_host_name.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            );

            let system_statuses: Vec<(
                FlakeReference,
                &ConfigInfo,
                Result<SystemStatus, NixError>,
            )> = deploy_infos
                .iter()
                .filter_map(|(system, deploy_result)| {
                    deploy_result
                        .as_ref()
                        .ok()
                        .map(|info| (system.clone(), info))
                })
                .par_bridge()
                .map(|(system, info)| {
                    (
                        system,
                        info,
                        check_system_status(Some(&info.fqdn_or_host_name)),
                    )
                })
                .collect();

            // Finally, print all results
            println!("\nSystem Status:");
            for (system, info, status) in system_statuses {
                println!("\n=== {} ===", system.to_string().cyan().bold());

                match status {
                    Ok(SystemStatus::Unreachable) => {
                        println!("  {} System not reachable", "✗".red());
                    }
                    Ok(SystemStatus::Reachable {
                        current_generation,
                        needs_reboot,
                        uptime_seconds,
                        failed_units,
                    }) => {
                        println!(
                            "  {} systemd units: {}",
                            passed_symbol(failed_units == 0),
                            if failed_units == 0 {
                                "all OK".to_string()
                            } else {
                                format!("{} failed", failed_units).to_string()
                            }
                        );

                        let generation_status = current_generation == info.toplevel_out;
                        println!(
                            "  {} System generation {}",
                            passed_symbol(generation_status),
                            if generation_status {
                                "up to date"
                            } else {
                                "outdated"
                            }
                        );

                        println!(
                            "  {} Reboot required: {}",
                            if needs_reboot {
                                "!".yellow().to_string()
                            } else {
                                "✓".green().to_string()
                            },
                            if needs_reboot { "yes" } else { "no" }
                        );

                        let days = uptime_seconds / 86400;
                        let hours = (uptime_seconds % 86400) / 3600;
                        let minutes = (uptime_seconds % 3600) / 60;
                        println!("    Uptime: {}d {}h {}m", days, hours, minutes);
                    }
                    Err(e) => println!("  {} Error getting system status: {}", "✗".red(), e),
                }
            }
        }
        Command::GenerateDocs { output_dir } => {
            create_dir_all(&output_dir)?;

            for group in get_standard_checks() {
                let filename = format!("{}/{}.md", output_dir, group.id);
                let mut content = String::new();

                // Add header
                content.push_str(&format!("# {}\n\n", group.name));
                content.push_str(&format!("{}\n\n", group.description));

                // Add table of contents
                content.push_str("## Checks\n\n");
                for check in &group.checks {
                    content.push_str(&format!(
                        "- [{} - {}](#{})\n",
                        check.id, check.description, check.id
                    ));
                }
                content.push('\n');

                // Add detailed check information
                content.push_str("## Details\n\n");
                for check in &group.checks {
                    content.push_str(&format!(
                        "### {}<a name=\"{}\"></a>\n\n",
                        check.id, check.id
                    ));
                    content.push_str("**Description:**\n");
                    content.push_str(&format!("{}\n\n", check.description));
                    content.push_str("**How to fix:**\n");
                    content.push_str(&format!("{}\n\n", check.advice));
                }

                fs::write(filename, content)?;
            }

            eprintln!("Documentation generated in {}", output_dir);
        }
    }
    Ok(())
}

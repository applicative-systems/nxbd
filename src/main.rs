mod cli;
mod libnxbd;

use crate::cli::{Cli, Command};
use clap::{CommandFactory, Parser};
use libnxbd::{
    configcheck::{
        get_standard_checks, load_ignored_checks, merge_ignore_maps, run_all_checks,
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
    EvaluationFails {
        failures: Vec<(FlakeReference, NixError)>,
    },
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
            Self::EvaluationFails { failures } => {
                writeln!(f, "The following configs have evaluation errors:")?;
                for (system, error) in failures {
                    writeln!(f, "  - {}: {}", system, error)?;
                }
                Ok(())
            }
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
    info: &ConfigInfo,
    user_info: &UserInfo,
    system_ignore_map: Option<&libnxbd::configcheck::IgnoreMap>,
) -> Result<Vec<(String, String)>, NixError> {
    let results = run_all_checks(info, user_info, system_ignore_map);
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

    match &cli.command {
        Command::GenerateDocs { output_dir } => {
            let app = Cli::command();

            create_dir_all(format!("{}/commands", output_dir))?;
            create_dir_all(format!("{}/checks", output_dir))?;

            {
                // CLI command overview for index.md
                let mut content = String::new();
                content.push_str("# `nxbd` CLI Commands\n\n");
                content.push_str(
                    "This section describes all available commands in the `nxbd` tool.\n\n",
                );

                content.push_str("## Available Commands\n\n");

                for subcmd in app.get_subcommands() {
                    if subcmd.is_hide_set() {
                        continue;
                    }

                    content.push_str(&format!("## `nxbd {}`\n\n", subcmd.get_name()));

                    if let Some(long_about) = subcmd.get_long_about() {
                        content.push_str(&format!("{}\n\n", long_about.to_string()));
                    }
                    content.push_str(&format!("[Details]({}.md)\n\n", subcmd.get_name()));
                }

                content.push_str("## Global Options\n\n");
                content.push_str("- `--verbose`: Show detailed information during execution\n\n");

                fs::write(format!("{}/commands/index.md", output_dir), content)?;
            }

            // Generate documentation for each command on an individual page
            for subcmd in app.get_subcommands() {
                let mut content = String::new();

                if subcmd.is_hide_set() {
                    continue;
                }

                content.push_str(&format!("# `nxbd {}`\n\n", subcmd.get_name()));

                if let Some(long_about) = subcmd.get_long_about() {
                    content.push_str(&format!("{}\n\n", long_about.to_string()));
                }

                let mut has_args = false;
                for arg in subcmd.get_arguments() {
                    if !has_args {
                        content.push_str("## Arguments\n");
                        has_args = true;
                    }

                    let arg_name = arg.get_id().as_str();
                    let help = arg
                        .get_help()
                        .map(|h| h.to_string())
                        .unwrap_or_else(|| "No description available".to_string());
                    let prefix = if arg.is_positional() { "" } else { "--" };
                    let value_hint = if !arg.is_positional()
                        && !arg.get_value_parser().possible_values().is_some()
                    {
                        let default_value = arg
                            .get_default_values()
                            .first()
                            .and_then(|v| v.to_str())
                            .map(|v| format!(" [default: {}]", v))
                            .unwrap_or_else(|| String::new());
                        format!(" <{}>{}", arg_name.to_lowercase(), default_value)
                    } else {
                        "".to_string()
                    };

                    content.push_str(&format!(
                        "### `{}{}{}`{}\n\n{}\n\n",
                        prefix,
                        arg_name,
                        value_hint,
                        if arg.is_required_set() {
                            ""
                        } else {
                            " (optional)"
                        },
                        help
                    ));
                }

                fs::write(
                    format!("{}/commands/{}.md", output_dir, subcmd.get_name()),
                    content,
                )?;
            }

            {
                // Check overview for checks/index.md
                let mut content = String::new();
                content.push_str("# `nxbd` NixOS Configuration Checks\n\n");
                for group in get_standard_checks() {
                    content.push_str(&format!("## {}\n\n", group.name));
                    content.push_str(&format!("{}\n\n", group.description));
                    content.push_str(&format!("[Details]({}.md)\n\n", group.id));
                }
                fs::write(format!("{}/checks/index.md", output_dir), content)?;
            }

            // Generate check documentation (existing code)
            for group in get_standard_checks() {
                let mut content = String::new();

                // Add header
                content.push_str(&format!("# {}\n\n", group.name));
                content.push_str(&format!("{}\n\n", group.description));

                // Add table of contents
                content.push_str("## Checks\n\n");
                for check in &group.checks {
                    content.push_str(&format!(
                        "- [`{}` - {}](#{})\n",
                        check.id, check.description, check.id
                    ));
                }
                content.push('\n');

                // Add detailed check information
                content.push_str("## Details\n\n");
                for check in &group.checks {
                    content.push_str(&format!(
                        "### `{}`<a name=\"{}\"></a>\n\n",
                        check.id, check.id
                    ));
                    content.push_str("**Description:**\n");
                    content.push_str(&format!("{}\n\n", check.description));
                    content.push_str("**How to fix:**\n");
                    content.push_str(&format!("{}\n\n", check.advice));
                }

                fs::write(format!("{}/checks/{}.md", output_dir, group.id), content)?;
            }

            eprintln!("Documentation generated in {}", output_dir);
            return Ok(());
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
            return Ok(());
        }
        _ => {}
    }

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
                eprintln!(
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
            // TODO: Build only locally buildable systems
            for system in &system_attributes {
                let result = nixos_deploy_info(system)?;
                eprintln!("{}", format!("→ Building system: {}", system).white());
                realise_toplevel_output_paths(&[system.clone()])?;
                eprintln!(
                    "{}",
                    format!("→ Built store path for {}: {}", system, result.toplevel_out).white()
                );
            }
        }
        Command::SwitchRemote {
            systems,
            ignore_checks,
            reboot,
            ignored_checks,
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

            // Check if any configurations had evaluation errors
            let evaluation_errors: Vec<(FlakeReference, NixError)> = deploy_infos
                .iter()
                .filter_map(|(system, result)| match result {
                    Err(err) => Some((system.clone(), err.clone())),
                    _ => None,
                })
                .collect();

            if !evaluation_errors.is_empty() {
                return Err(NxbdError::EvaluationFails {
                    failures: evaluation_errors,
                });
            }

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
                // Load ignored checks from file
                let ignored_checks_map = load_ignored_checks(".nxbd-ignore.yaml");

                let mut all_failures = Vec::new();
                for (system, info) in &deploy_infos {
                    match info {
                        Ok(info) => {
                            // Extract the right ignore map for the current system
                            let mut system_ignore_map = ignored_checks_map
                                .as_ref()
                                .and_then(|map| map.get(&system.attribute))
                                .cloned();

                            // Merge with command line ignored checks if provided
                            if let Some(cmd_ignores) = &ignored_checks {
                                system_ignore_map = if let Some(map) = system_ignore_map {
                                    Some(merge_ignore_maps(&map, cmd_ignores))
                                } else {
                                    Some(cmd_ignores.clone())
                                };
                            }

                            let failures =
                                run_system_checks(info, &user_info, system_ignore_map.as_ref())?;
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
            ignored_checks,
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
                // Load ignored checks once
                let ignored_checks_map = load_ignored_checks(".nxbd-ignore.yaml");

                // Extract the right ignore map for the current system
                let mut system_ignore_map = ignored_checks_map
                    .as_ref()
                    .and_then(|map| map.get(&system_attribute.attribute))
                    .cloned();

                // Merge with command line ignored checks if provided
                if let Some(cmd_ignores) = &ignored_checks {
                    system_ignore_map = if let Some(map) = system_ignore_map {
                        Some(merge_ignore_maps(&map, cmd_ignores))
                    } else {
                        Some(cmd_ignores.clone())
                    };
                }

                let failures =
                    run_system_checks(&deploy_info, &user_info, system_ignore_map.as_ref())?;
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
            ignored_checks,
        } => {
            let system_attributes = flakerefs_or_default(systems)?;
            let file_ignored_checks = load_ignored_checks(&ignore_file);

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
                        // Extract the right ignore map for the current system
                        let mut system_ignore_map = file_ignored_checks
                            .as_ref()
                            .and_then(|map| map.get(&system.attribute))
                            .cloned();

                        // Merge with command line ignored checks if provided
                        if let Some(cmd_ignores) = &ignored_checks {
                            system_ignore_map = if let Some(map) = system_ignore_map {
                                Some(merge_ignore_maps(&map, cmd_ignores))
                            } else {
                                Some(cmd_ignores.clone())
                            };
                        }

                        (
                            system,
                            run_all_checks(i, &user_info, system_ignore_map.as_ref()),
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
                        if !cli.verbose && check_result.passed {
                            continue;
                        }
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
        Command::Checks => {}
        Command::GenerateDocs { output_dir: _ } => {}
    }
    Ok(())
}

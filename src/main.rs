mod cli;
mod libnxbd;

use crate::cli::{Cli, Command};
use clap::Parser;
use libnxbd::{
    configcheck::{
        get_standard_checks, is_check_ignored, load_ignored_checks, run_all_checks,
        save_failed_checks_to_ignore_file,
    },
    nixcommands::{
        activate_profile, check_needs_reboot, copy_to_host, nixos_configuration_flakerefs,
        realise_drv_remotely, realise_toplevel_output_path, switch_to_configuration,
    },
    nixosattributes::{nixos_deploy_info, ConfigInfo},
    userinfo::UserInfo,
    FlakeReference, NixError,
};
use nix::unistd;
use owo_colors::OwoColorize;
use std::fmt;

#[derive(Debug)]
enum NxbdError {
    ChecksFailed {
        failures: Vec<(FlakeReference, Vec<(String, String)>)>, // (system, [(group_id, check_id)])
        is_switch: bool,
    },
    Nix(NixError),
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
            Self::Nix(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for NxbdError {}

impl From<NixError> for NxbdError {
    fn from(err: NixError) -> Self {
        NxbdError::Nix(err)
    }
}

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

fn run_system_checks(
    system: &FlakeReference,
    info: &ConfigInfo,
    user_info: &UserInfo,
    ignore_file: &str,
) -> Result<Vec<(String, String)>, NixError> {
    let ignored_checks = load_ignored_checks(ignore_file);
    let results = run_all_checks(info, user_info);
    let mut failures = Vec::new();

    for (group_id, _, check_results) in &results {
        for (check_id, passed) in check_results {
            if !passed
                && !ignored_checks
                    .as_ref()
                    .map(|ic| is_check_ignored(ic, &system, group_id, check_id))
                    .unwrap_or(false)
            {
                failures.push((group_id.clone(), check_id.clone()));
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
        } => {
            let system_attributes = flakerefs_or_default(systems)?;
            println!(
                "Switching systems: {}",
                system_attributes
                    .iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<String>>()
                    .join(" ")
            );

            // Get all deploy infos first
            let deploy_infos: Vec<(FlakeReference, Result<ConfigInfo, NixError>)> =
                system_attributes
                    .iter()
                    .map(|sa| (sa.clone(), nixos_deploy_info(sa)))
                    .collect();

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

            // Do the deployment using the already fetched deploy infos
            let results: Vec<(FlakeReference, Result<(), NixError>)> = deploy_infos
                .iter()
                .map(|(sa, info_result)| {
                    let result =
                        info_result
                            .as_ref()
                            .map_err(|e| e.clone())
                            .and_then(|deploy_info| {
                                deploy_remote(
                                    sa,
                                    &deploy_info.toplevel_out,
                                    &deploy_info.toplevel_drv,
                                    &deploy_info.fqdn_or_host_name,
                                    !user_info.can_build_natively(&deploy_info.system),
                                )
                            });
                    (sa.clone(), result)
                })
                .collect();

            println!("\nDeployment Summary:");
            for (system, result) in results {
                match result {
                    Ok(()) => {
                        let reboot_status = deploy_infos
                            .iter()
                            .find(|(s, _)| s == &system)
                            .and_then(|(_, i)| i.as_ref().ok())
                            .and_then(|info| check_needs_reboot(Some(&info.fqdn_or_host_name)).ok())
                            .map_or("", |needs_reboot| {
                                if needs_reboot {
                                    " (reboot required)"
                                } else {
                                    ""
                                }
                            });

                        println!("  {} {}{}", "✓".green(), system, reboot_status);
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
                    return Err(NxbdError::ChecksFailed {
                        failures: vec![(system_attribute.clone(), vec![(
                            "hostname".to_string(),
                            format!("system config has '{}' but local system is '{}'. Use --ignore-hostname to ignore this check.",
                            config_hostname, local_hostname)
                        )])],
                        is_switch: true,
                    });
                }
            }

            let toplevel = realise_toplevel_output_path(&system_attribute)?;
            // We should change this in a way that realise_toplevel_output_path actually accepts the .drv file, but that may be impeding to nix-output-monitor
            assert_eq!(
                toplevel, deploy_info.toplevel_out,
                "Built output path does not match evaluated output path"
            );

            println!("Store path is [{toplevel}]");
            activate_profile(&toplevel, true, None)?;
            switch_to_configuration(&toplevel, "switch", true, None)?;

            match check_needs_reboot(None) {
                Ok(true) => println!("System update complete. Reboot required."),
                Ok(false) => println!("System update complete."),
                Err(_) => println!("System update complete. Reboot status unknown."),
            }
        }
        Command::Check {
            systems,
            save_ignore,
            ignore_file,
        } => {
            let system_attributes = flakerefs_or_default(systems)?;
            println!("\nSystem Configurations:");

            let ignored_checks = load_ignored_checks(&ignore_file);
            let mut all_results = Vec::new();
            let mut had_failures = false;

            for system in &system_attributes {
                println!("\n=== {} ===", system.to_string().cyan().bold());
                match nixos_deploy_info(system) {
                    Ok(info) => {
                        let results = run_all_checks(&info, &user_info);

                        // Check for unignored failures while displaying results
                        for (group_id, _, check_results) in &results {
                            for (check_id, passed) in check_results {
                                if !passed
                                    && !ignored_checks
                                        .as_ref()
                                        .map(|ic| is_check_ignored(ic, system, group_id, check_id))
                                        .unwrap_or(false)
                                {
                                    had_failures = true;
                                }
                            }
                        }

                        let results_for_display = results.clone();

                        for (group_id, _, check_results) in results_for_display {
                            if let Some(group) =
                                get_standard_checks().into_iter().find(|g| g.id == group_id)
                            {
                                // First, separate passed and failed checks
                                let (passed_checks, failed_checks): (Vec<_>, Vec<_>) =
                                    check_results.into_iter().partition(|(_, passed)| *passed);

                                // Filter failed checks to remove ignored ones
                                let unignored_failures: Vec<_> = failed_checks
                                    .into_iter()
                                    .filter(|(check_id, _)| {
                                        !ignored_checks
                                            .as_ref()
                                            .map(|ic| {
                                                is_check_ignored(ic, system, &group_id, check_id)
                                            })
                                            .unwrap_or(false)
                                    })
                                    .collect();

                                // Only show group if there are any non-ignored checks
                                let non_ignored_count =
                                    passed_checks.len() + unignored_failures.len();
                                if non_ignored_count > 0 {
                                    // Group passes if there are no unignored failures
                                    let effective_group_passed = unignored_failures.is_empty();

                                    println!(
                                        "{} - {}: {} {}\n{}\n",
                                        group.id.cyan().bold(),
                                        group.name.bold(),
                                        passed_symbol(effective_group_passed),
                                        format!("({}/{})", passed_checks.len(), non_ignored_count)
                                            .dimmed(),
                                        group.description.dimmed()
                                    );

                                    // Show all passed checks
                                    for (check_id, _) in passed_checks {
                                        println!("  {}: {}", check_id, passed_symbol(true));
                                    }

                                    // Show unignored failed checks
                                    for (check_id, check_passed) in unignored_failures {
                                        println!("  {}: {}", check_id, passed_symbol(check_passed));
                                        if cli.verbose {
                                            if let Some(check) =
                                                group.checks.iter().find(|c| c.id == check_id)
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
                                println!();
                            }
                        }
                        all_results.push((system, results));
                    }
                    Err(e) => {
                        had_failures = true;
                        match e {
                            NixError::Eval(msg) => {
                                println!("Error evaluating system info:\n{}", msg)
                            }
                            _ => println!("Error getting system info: {:?}", e),
                        }
                    }
                }
            }

            if *save_ignore {
                if let Err(e) = save_failed_checks_to_ignore_file(&ignore_file, &all_results) {
                    eprintln!("Failed to save ignore file: {}", e);
                } else {
                    println!("Created {} with failed checks", ignore_file);
                }
            }

            if had_failures {
                return Err(NxbdError::ChecksFailed {
                    failures: Vec::new(),
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
    }
    Ok(())
}

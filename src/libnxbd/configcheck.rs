use super::FlakeReference;
use super::{nixosattributes::ConfigInfo, userinfo::UserInfo};
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::str::FromStr;

#[derive(Debug)]
pub struct CheckError {
    pub check_name: String,
    pub message: String,
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.check_name, self.message)
    }
}

#[allow(clippy::struct_field_names, clippy::type_complexity)]
pub struct Check {
    pub id: String,
    pub description: String,
    pub advice: String,
    check_fn: Box<dyn Fn(&ConfigInfo, &UserInfo) -> Result<(), CheckError>>,
}

impl Check {
    pub fn new<F>(id: &str, description: &str, advice: &str, check_fn: F) -> Self
    where
        F: Fn(&ConfigInfo, &UserInfo) -> Result<(), CheckError> + 'static,
    {
        Check {
            id: id.to_string(),
            description: description.to_string(),
            advice: advice.to_string(),
            check_fn: Box::new(check_fn),
        }
    }

    pub fn check(&self, config: &ConfigInfo, user_info: &UserInfo) -> Result<(), CheckError> {
        (self.check_fn)(config, user_info)
    }
}

pub struct CheckGroup {
    pub id: String,
    pub name: String,
    pub description: String,
    pub checks: Vec<Check>,
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub id: String,
    pub description: String,
    pub advice: String,
    pub passed: bool,
    pub ignored: bool,
}

#[derive(Debug, Clone)]
pub struct CheckGroupResult {
    pub id: String,
    pub name: String,
    pub description: String,
    pub checks: Vec<CheckResult>,
}

/// Map of group IDs to check IDs to ignore
///
/// A key with an empty vector means "ignore all checks in this group"
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IgnoreMap(pub HashMap<String, Vec<String>>);

impl IgnoreMap {
    pub fn new() -> Self {
        IgnoreMap(HashMap::new())
    }

    pub fn insert(&mut self, key: String, value: Vec<String>) -> Option<Vec<String>> {
        self.0.insert(key, value)
    }

    pub fn get(&self, key: &str) -> Option<&Vec<String>> {
        self.0.get(key)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromStr for IgnoreMap {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_ignore_string(s).map_err(|e| e.to_string())
    }
}

impl<'a> IntoIterator for &'a IgnoreMap {
    type Item = (&'a String, &'a Vec<String>);
    type IntoIter = std::collections::hash_map::Iter<'a, String, Vec<String>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

pub fn run_all_checks(
    config: &ConfigInfo,
    user_info: &UserInfo,
    ignored_checks: Option<&IgnoreMap>,
) -> Vec<CheckGroupResult> {
    get_standard_checks()
        .iter()
        .map(|group| {
            let check_results: Vec<CheckResult> = group
                .checks
                .iter()
                .map(|check| {
                    let passed = check.check(config, user_info).is_ok();
                    // A check is ignored if:
                    // 1. It's failed (not passed) AND
                    // 2. Either:
                    //    a. It's in a group with an empty vector in the ignore map (ignore all in group)
                    //    b. It's specifically listed in the ignore map
                    let ignored = !passed
                        && ignored_checks
                            .and_then(|system_map| system_map.get(&group.id))
                            .is_some_and(|checks| checks.is_empty() || checks.contains(&check.id));

                    CheckResult {
                        id: check.id.clone(),
                        description: check.description.clone(),
                        advice: check.advice.clone(),
                        passed,
                        ignored,
                    }
                })
                .collect();

            CheckGroupResult {
                id: group.id.clone(),
                name: group.name.clone(),
                description: group.description.clone(),
                checks: check_results,
            }
        })
        .collect()
}

#[allow(
    clippy::too_many_lines,
    clippy::if_not_else,
    clippy::struct_field_names,
    clippy::type_complexity
)]
pub fn get_standard_checks() -> Vec<CheckGroup> {
    vec![
        CheckGroup {
            id: "remote_deployment".to_string(),
            name: "Remote Deployment Support".to_string(),
            description: "Checks if the system has the required configuration to safely perform remote deployments. This avoids a lock-out after the deployment.".to_string(),
            checks: vec![
                Check::new(
                    "ssh_enabled",
                    "SSH service must be enabled",
                    "Set  `services.openssh.enable = true`",
                    |config, _user_info| {
                        if !config.ssh_enabled {
                            Err(CheckError {
                                check_name: "SSH".to_string(),
                                message: "SSH service is not enabled".to_string(),
                            })
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "sudo_enabled",
                    "Sudo must be available",
                    "Set `security.sudo.enable = true`",
                    |config, _user_info| {
                        if !config.sudo_enabled {
                            Err(CheckError {
                                check_name: "Sudo".to_string(),
                                message: "Sudo is not enabled".to_string(),
                            })
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "wheel_passwordless",
                    "Wheel group should not require password for sudo",
                    "Set  `security.sudo.wheelNeedsPassword = false`",
                    |config, _user_info| {
                        if config.wheel_needs_password {
                            Err(CheckError {
                                check_name: "Sudo Password".to_string(),
                                message: "Wheel group members need password for sudo".to_string(),
                            })
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "nix_trusts_wheel",
                    "Wheel group must be trusted by Nix",
                    "Add `@wheel` to `nix.settings.trusted-users`",
                    |config, _user_info| {
                        if !config.nix_trusts_wheel {
                            Err(CheckError {
                                check_name: "Nix Trust".to_string(),
                                message: "`wheel` group is not trusted by nix".to_string(),
                            })
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "user_access",
                    "Current user must have SSH access",
                    "Add your SSH key to the user's authorized_keys",
                    |config, user_info| {
                        let current_user = &user_info.username;
                        match config.users.iter().find(|u| u.name == *current_user) {
                            None => Err(CheckError {
                                check_name: "User Access".to_string(),
                                message: format!("User '{}' does not exist on target system", current_user),
                            }),
                            Some(user) => {
                                let has_matching_key = user_info
                                    .ssh_keys
                                    .iter()
                                    .any(|local_key| user.ssh_keys.contains(&local_key));

                                if !has_matching_key {
                                    Err(CheckError {
                                        check_name: "User Access".to_string(),
                                        message: format!(
                                            "User '{}' exists but none of their local SSH keys are authorized",
                                            current_user
                                        ),
                                    })
                                } else {
                                    Ok(())
                                }
                            }
                        }
                    },
                ),
                Check::new(
                    "user_in_wheel",
                    "Current user must be in wheel group",
                    "Add your user to the wheel group",
                    |config, user_info| {
                        let current_user = &user_info.username;
                        match config.users.iter().find(|u| u.name == *current_user) {
                            None => Err(CheckError {
                                check_name: "Wheel Group".to_string(),
                                message: format!("User '{}' does not exist on target system", current_user),
                            }),
                            Some(user) => {
                                if !user.extra_groups.contains(&"wheel".to_string()) {
                                    Err(CheckError {
                                        check_name: "Wheel Group".to_string(),
                                        message: format!(
                                            "User '{}' is not in the wheel group",
                                            current_user
                                        ),
                                    })
                                } else {
                                    Ok(())
                                }
                            }
                        }
                    },
                ),
            ],
        },
        CheckGroup {
            id: "system_security".to_string(),
            name: "System Security Settings".to_string(),
            description: "Checks if critical system security settings are properly configured".to_string(),
            checks: vec![
                Check::new(
                    "wheel_only",
                    "Only wheel group members should be allowed to use sudo",
                    "Set  `security.sudo.execWheelOnly = true`",
                    |config, _user_info| {
                        if !config.sudo_wheel_only {
                            Err(CheckError {
                                check_name: "Sudo Wheel Only".to_string(),
                                message: "Users outside wheel group can use sudo".to_string(),
                            })
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "ssh_password_authentication",
                    "Password authentication should be disabled for SSH",
                    "Set  `services.openssh.settings.PasswordAuthentication = false`",
                    |config, _user_info| {
                        if config.ssh_password_authentication {
                            Err(CheckError {
                                check_name: "SSH Password Auth".to_string(),
                                message: "SSH password authentication is enabled. Consider disabling it and using only key-based authentication for better security".to_string(),
                            })
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "users_immutable",
                    "Users should be managed through NixOS configuration",
                    "Set  `users.mutableUsers = false`",
                    |config, _user_info| {
                        if config.users_mutable {
                            Err(CheckError {
                                check_name: "Mutable Users".to_string(),
                                message: "Users can be modified outside of the NixOS configuration. Consider setting  `users.mutableUsers = false` for better system reproducibility".to_string(),
                            })
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "firewall_enabled",
                    "The system firewall should be enabled for better security",
                    "Set  `networking.firewall.enable = true`",
                    |config, _user_info| {
                        if !config.networking_firewall_enabled {
                            Err(CheckError {
                                check_name: "Firewall".to_string(),
                                message: "System firewall is not enabled. Consider setting  `networking.firewall.enable = true`".to_string(),
                            })
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "log_refused_connections",
                    "The logging of refused connections should be deactivated to avoid flooding the logs and possibly leaving important messages unseen. Consider using it only for debugging firewall rules.",
                    "Set  `networking.firewall.logRefusedConnections = false`",
                    |config, _user_info| {
                        if config.log_refused_connections {
                            Err(CheckError {
                                check_name: "Log refused connections".to_string(),
                                message: "Logging of refused connections should be disabled. Consider setting  `networking.firewall.logRefusedConnections = false`".to_string(),
                            })
                        } else {
                            Ok(())
                        }
                    },
                ),
            ],
        },
        CheckGroup {
            id: "system_maintenance".to_string(),
            name: "System Maintenance Settings".to_string(),
            description: "Checks if system maintenance and cleanup settings are properly configured".to_string(),
            checks: vec![
                Check::new(
                    "system_generations_limit",
                    "The retention of old system generations should be limited, as these are protected from garbage collection and consume disk space unnecessarily.",
                    "Set `boot.systemd.generations = 10` or less for systemd-boot, or `boot.grub.generations = 10` or less for GRUB",
                    |config, _user_info| {
                        fn check_generations(enabled: bool, limit: Option<i32>, bootloader: &str) -> Result<(), CheckError> {
                            if !enabled {
                                return Ok(());
                            }
                            match limit {
                                Some(limit) if limit > 10 => Err(CheckError {
                                    check_name: "Boot Generations".to_string(),
                                    message: format!(
                                        "Too many {} generations kept ({}). Consider reducing to 10 or less",
                                        bootloader, limit
                                    ),
                                }),
                                None => Err(CheckError {
                                    check_name: "Boot Generations".to_string(),
                                    message: format!(
                                        "No {} generation limit set. This may prevent old generations from being garbage collected",
                                        bootloader
                                    ),
                                }),
                                _ => Ok(()),
                            }
                        }

                        check_generations(config.boot_systemd, config.boot_systemd_generations, "systemd-boot")
                            .or_else(|_| check_generations(config.boot_grub, config.boot_grub_generations, "GRUB"))
                    },
                ),
                Check::new(
                    "nix_gc",
                    "Regular Nix Garbage Collection should be enabled",
                    "Set  `nix.gc.automatic = true`",
                    |config, _user_info| {
                        if !config.nix_gc {
                            Err(CheckError {
                                check_name: "Garbage Collection".to_string(),
                                message: "Garbage Collection is not enabled. Consider setting  `nix.gc.automatic = true`".to_string(),
                            })
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "nix_optimise_automatic",
                    "Nix store optimisation should be enabled",
                    "Set either `nix.settings.auto-optimise-store` or `nix.optimise.automatic`",
                    |config, _user_info| {
                        if config.boot_is_container {
                            Ok(())
                        } else if !config.nix_optimise_automatic && !config.nix_auto_optimise_store {
                            Err(CheckError {
                                check_name: "Nix store optimisation".to_string(),
                                message: "Nix store optimisation is disabled. Set either `nix.settings.auto-optimise-store` or `nix.optimise.automatic`".to_string(),
                            })
                        } else {
                            Ok(())
                        }
                    },
                ),
            ],
        },
        CheckGroup {
            id: "nix_configuration".to_string(),
            name: "Nix Configuration".to_string(),
            description: "Checks if Nix is configured with recommended settings".to_string(),
            checks: vec![
                Check::new(
                    "nix_extra_options",
                    "Nix features should include nix-command and flakes",
                    "Add 'nix-command flakes' to nix.settings.experimental-features",
                    |config, _user_info| {
                        let features_line = config.nix_extra_options
                            .lines()
                            .find(|line| line.trim().starts_with("experimental-features"))
                            .unwrap_or("");
                        if !features_line.contains("nix-command")
                            && !config.nix_settings_experimental_features.contains("nix-command") {
                            Err(CheckError {
                                check_name: "Nix Features".to_string(),
                                message: "Missing required nix feature 'nix-command'. Add it to experimental-features in nix.extraOptions".to_string(),
                            })
                        } else if !features_line.contains("flakes")
                            && !config.nix_settings_experimental_features.contains("flakes") {
                            Err(CheckError {
                                check_name: "Nix Features".to_string(),
                                message: "Missing required nix feature 'flakes'. Add it to experimental-features in nix.extraOptions".to_string(),
                            })
                        } else {
                            Ok(())
                        }
                    },
                ),
            ],
        },
        CheckGroup {
            id: "server_optimization".to_string(),
            name: "Server Optimization Settings".to_string(),
            description: "Checks if server-specific optimizations are properly configured".to_string(),
            checks: vec![
                Check::new(
                    "doc_nixos",
                    "NixOS documentation should be disabled to reduce system closure size",
                    "Set  `documentation.nixos.enable = false`",
                    |config, _user_info| {
                        if config.fqdn.is_some() {
                            if config.doc_nixos_enabled {
                                Err(CheckError {
                                    check_name: "Documentation".to_string(),
                                    message: "NixOS documentation enabled. Consider setting  `documentation.nixos.enable = false`".to_string(),
                                })
                            } else {
                                Ok(())
                            }
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "documentation",
                    "General documentation should be disabled to reduce system closure size",
                    "Set  `documentation.enable = false`",
                    |config, _user_info| {
                        if config.fqdn.is_some() {
                            if config.doc_enable {
                                Err(CheckError {
                                    check_name: "Documentation".to_string(),
                                    message: "General documentation enabled. Consider setting  `documentation.enable = false`".to_string(),
                                })
                            } else {
                                Ok(())
                            }
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "doc_dev",
                    "Development documentation should be disabled to reduce system closure size",
                    "Set  `documentation.dev.enable = false`",
                    |config, _user_info| {
                        if config.fqdn.is_some() {
                            if config.doc_dev_enable {
                                Err(CheckError {
                                    check_name: "Documentation".to_string(),
                                    message: "Development documentation enabled. Consider setting  `documentation.dev.enable = false`".to_string(),
                                })
                            } else {
                                Ok(())
                            }
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "doc_doc",
                    "Doc documentation should be disabled to reduce system closure size",
                    "Set  `documentation.doc.enable = false`",
                    |config, _user_info| {
                        if config.fqdn.is_some() {
                            if config.doc_doc_enable {
                                Err(CheckError {
                                    check_name: "Documentation".to_string(),
                                    message: "Doc documentation enabled. Consider setting  `documentation.doc.enable = false`".to_string(),
                                })
                            } else {
                                Ok(())
                            }
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "doc_info",
                    "Info documentation should be disabled to reduce system closure size",
                    "Set  `documentation.info.enable = false`",
                    |config, _user_info| {
                        if config.fqdn.is_some() {
                            if config.doc_info_enable {
                                Err(CheckError {
                                    check_name: "Documentation".to_string(),
                                    message: "Info documentation enabled. Consider setting  `documentation.info.enable = false`".to_string(),
                                })
                            } else {
                                Ok(())
                            }
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "doc_man",
                    "Man pages should be disabled to reduce system closure size",
                    "Set  `documentation.man.enable = false`",
                    |config, _user_info| {
                        if config.fqdn.is_some() {
                            if config.doc_man_enable {
                                Err(CheckError {
                                    check_name: "Documentation".to_string(),
                                    message: "Man pages enabled. Consider setting  `documentation.man.enable = false`".to_string(),
                                })
                            } else {
                                Ok(())
                            }
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "fontconfig",
                    "Font configuration should be disabled on servers to reduce system closure size",
                    "Set  `fonts.fontconfig.enable = false`",
                    |config, _user_info| {
                        if config.fqdn.is_some() && config.font_fontconfig_enable{
                            Err(CheckError {
                                check_name: "Font Configuration".to_string(),
                                message: "Font configuration is enabled. Consider setting  `fonts.fontconfig.enable = false` on servers".to_string(),
                            })
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "stub_ld",
                    "Stub-ld is typically not needed on servers and increases system closure size",
                    "Set  `environment.stub-ld.enable = false`",
                    |config, _user_info| {
                        if config.fqdn.is_some() && config.stub_ld {
                            Err(CheckError {
                                check_name: "Stub LD".to_string(),
                                message: "Stub-ld is enabled but typically not needed on servers. Consider setting  `environment.stub-ld.enable = false` to reduce system closure size".to_string(),
                            })
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "command_not_found",
                    "The command-not-found program is typically not needed on servers and increases system closure size",
                    "Set  `programs.command-not-found.enable = false`",
                    |config, _user_info| {
                        if config.fqdn.is_some() && config.command_not_found {
                            Err(CheckError {
                                check_name: "Command Not Found".to_string(),
                                message: "The command-not-found program is enabled but typically not needed on servers. Consider setting  `programs.command-not-found.enable = false` to reduce system closure size".to_string(),
                            })
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "nginx_brotli",
                    "Brotli compression should be enabled",
                    "Set  `services.nginx.recommendedBrotliSettings = true`",
                    |config, _user_info| {
                        if config.nginx_enabled {
                            if !config.nginx_brotli {
                                Err(CheckError {
                                    check_name: "Nginx Settings".to_string(),
                                    message: "Brotli compression not enabled. Consider setting  `services.nginx.recommendedBrotliSettings = true`".to_string(),
                                })
                            } else {
                                Ok(())
                            }
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "nginx_gzip",
                    "Gzip compression should be enabled",
                    "Set  `services.nginx.recommendedGzipSettings = true`",
                    |config, _user_info| {
                        if config.nginx_enabled {
                            if !config.nginx_gzip {
                                Err(CheckError {
                                    check_name: "Nginx Settings".to_string(),
                                    message: "Gzip compression not enabled. Consider setting  `services.nginx.recommendedGzipSettings = true`".to_string(),
                                })
                            } else {
                                Ok(())
                            }
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "nginx_optimisation",
                    "Optimisation settings should be enabled",
                    "Set  `services.nginx.recommendedOptimisation = true`",
                    |config, _user_info| {
                        if config.nginx_enabled {
                            if !config.nginx_optimisation {
                                Err(CheckError {
                                    check_name: "Nginx Settings".to_string(),
                                    message: "Optimisation settings not enabled. Consider setting  `services.nginx.recommendedOptimisation = true`".to_string(),
                                })
                            } else {
                                Ok(())
                            }
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "nginx_proxy",
                    "Proxy settings should be enabled",
                    "Set  `services.nginx.recommendedProxySettings = true`",
                    |config, _user_info| {
                        if config.nginx_enabled {
                            if !config.nginx_proxy {
                                Err(CheckError {
                                    check_name: "Nginx Settings".to_string(),
                                    message: "Proxy settings not enabled. Consider setting  `services.nginx.recommendedProxySettings = true`".to_string(),
                                })
                            } else {
                                Ok(())
                            }
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "nginx_tls",
                    "TLS settings should be enabled",
                    "Set  `services.nginx.recommendedTlsSettings = true`",
                    |config, _user_info| {
                        if config.nginx_enabled {
                            if !config.nginx_tls {
                                Err(CheckError {
                                    check_name: "Nginx Settings".to_string(),
                                    message: "TLS settings not enabled. Consider setting  `services.nginx.recommendedTlsSettings = true`".to_string(),
                                })
                            } else {
                                Ok(())
                            }
                        } else {
                            Ok(())
                        }
                    },
                ),
            ],
        },
        CheckGroup {
            id: "hardware_configuration".to_string(),
            name: "Hardware Configuration".to_string(),
            description: "Checks if hardware-specific settings are properly configured".to_string(),
            checks: vec![
                Check::new(
                    "cpu_microcode",
                    "CPU microcode updates should be enabled on Intel architecture",
                    "Set either `hardware.cpu.intel.updateMicrocode` or `hardware.cpu.amd.updateMicrocode`",
                    |config, _user_info| {
                        if config.is_x86 {
                            if !config.intel_microcode && !config.amd_microcode {
                                Err(CheckError {
                                    check_name: "Microcode".to_string(),
                                    message: "No CPU microcode updates enabled. Set either `hardware.cpu.intel.updateMicrocode` or `hardware.cpu.amd.updateMicrocode` to `true`".to_string(),
                                })
                            } else {
                                Ok(())
                            }
                        } else {
                            Ok(())
                        }
                    },
                ),
            ],
        },
    ]
}

#[derive(Debug)]
pub enum CheckFileError {
    Io(std::io::Error),
    Yaml(serde_yaml::Error),
}

impl std::fmt::Display for CheckFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {}", e),
            Self::Yaml(e) => write!(f, "YAML error: {}", e),
        }
    }
}

impl From<std::io::Error> for CheckFileError {
    fn from(err: std::io::Error) -> Self {
        CheckFileError::Io(err)
    }
}

impl From<serde_yaml::Error> for CheckFileError {
    fn from(err: serde_yaml::Error) -> Self {
        CheckFileError::Yaml(err)
    }
}

pub fn save_failed_checks_to_ignore_file(
    path: &str,
    system_results: &[(&FlakeReference, Vec<CheckGroupResult>)],
) -> Result<(), CheckFileError> {
    // Start with existing ignored checks if available
    let mut ignore_map = load_ignored_checks(path).unwrap_or_else(|| HashMap::new());

    // Update map with new results
    for (system, results) in system_results {
        let mut system_map_inner = HashMap::new();

        for group in results {
            let failed_checks: Vec<String> = group
                .checks
                .iter()
                .filter(|check| !check.passed)
                .map(|check| check.id.clone())
                .collect();

            if !failed_checks.is_empty() {
                system_map_inner.insert(group.id.clone(), failed_checks);
            }
        }

        if !system_map_inner.is_empty() {
            // Replace or insert the system's ignored checks
            ignore_map.insert(system.attribute.clone(), IgnoreMap(system_map_inner));
        } else {
            // If no failures for this system, remove it from ignored checks
            ignore_map.remove(&system.attribute);
        }
    }

    if !ignore_map.is_empty() {
        let yaml = serde_yaml::to_string(&ignore_map)?;
        fs::write(path, yaml)?;
    }

    Ok(())
}

pub fn load_ignored_checks(path: &str) -> Option<HashMap<String, IgnoreMap>> {
    match fs::read_to_string(path) {
        Ok(contents) => serde_yaml::from_str(&contents).ok(),
        Err(_) => None,
    }
}

/// Error type for parse_ignore_string
#[derive(Debug)]
pub enum ParseIgnoreError {
    /// No group specified (missing `.` in item)
    NoGroup(String),
    /// Empty group name
    EmptyGroup(String),
    /// Empty check name (except when using wildcard)
    EmptyCheck(String),
}

impl fmt::Display for ParseIgnoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoGroup(item) => write!(f, "No group specified in '{}' (missing '.')", item),
            Self::EmptyGroup(item) => write!(f, "Empty group name in '{}'", item),
            Self::EmptyCheck(item) => write!(f, "Empty check name in '{}'", item),
        }
    }
}

/// Parses a comma-separated list of group.check or group.* items into an IgnoreMap
///
/// Format: group1.check1,group2.check2,group3.*
///
/// Examples:
/// - `remote_deployment.ssh_enabled` - Ignore specific check
/// - `hardware_configuration.*` - Ignore all checks in group (empty vector)
/// - `group1.check1,group2.*` - Multiple ignore rules
///
/// Returns an IgnoreMap where:
/// - Keys are group IDs
/// - Values are lists of check IDs to ignore
/// - An empty vector means "ignore all checks in this group" (wildcard)
pub fn parse_ignore_string(s: &str) -> Result<IgnoreMap, ParseIgnoreError> {
    let mut inner_map = HashMap::new();

    // Split by commas
    for item in s.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue; // Skip empty items
        }

        // Each item should be in the format "group.check" or "group.*"
        let parts: Vec<&str> = item.split('.').collect();

        if parts.len() != 2 {
            return Err(ParseIgnoreError::NoGroup(item.to_string()));
        }

        let group = parts[0].trim();
        let check = parts[1].trim();

        if group.is_empty() {
            return Err(ParseIgnoreError::EmptyGroup(item.to_string()));
        }

        if check.is_empty() {
            return Err(ParseIgnoreError::EmptyCheck(item.to_string()));
        }

        // Handle wildcards
        if check == "*" {
            // Empty vector means "ignore all checks in this group"
            inner_map.insert(group.to_string(), vec![]);
        } else {
            // Add the specific check to the group's ignore list
            inner_map
                .entry(group.to_string())
                .or_insert_with(Vec::new)
                .push(check.to_string());
        }
    }

    Ok(IgnoreMap(inner_map))
}

/// Merges two IgnoreMaps into a new IgnoreMap
///
/// Rules for merging:
/// 1. If a group exists in only one map, it is copied to the result
/// 2. If a group exists in both maps:
///    a. If either map has an empty vector for the group (ignore all), the result has an empty vector
///    b. Otherwise, the result has the union of the checks from both maps
///
/// # Examples
///
/// ```
/// let map1 = parse_ignore_string("group1.check1,group2.*").unwrap();
/// let map2 = parse_ignore_string("group1.check2,group3.check3").unwrap();
///
/// // Result will have:
/// // - group1: [check1, check2]  (union)
/// // - group2: []                (empty vector preserved)
/// // - group3: [check3]          (from map2)
/// let merged = merge_ignore_maps(&map1, &map2);
/// ```
pub fn merge_ignore_maps(map1: &IgnoreMap, map2: &IgnoreMap) -> IgnoreMap {
    let mut result = IgnoreMap::new();

    // First, process all groups from map1
    for (group, checks) in map1 {
        if checks.is_empty() {
            // If map1 has an empty vector (ignore all), preserve it in the result
            result.insert(group.clone(), vec![]);
        } else if let Some(other_checks) = map2.get(group) {
            if other_checks.is_empty() {
                // If map2 has an empty vector (ignore all), prefer it
                result.insert(group.clone(), vec![]);
            } else {
                // Both maps have specific checks, merge them
                let mut merged_checks = checks.clone();
                // Add checks from map2 that aren't already in the result
                for check in other_checks {
                    if !merged_checks.contains(check) {
                        merged_checks.push(check.clone());
                    }
                }
                result.insert(group.clone(), merged_checks);
            }
        } else {
            // Group only exists in map1, copy it
            result.insert(group.clone(), checks.clone());
        }
    }

    // Then add any groups from map2 that weren't in map1
    for (group, checks) in map2 {
        if !result.contains_key(group) {
            result.insert(group.clone(), checks.clone());
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::{merge_ignore_maps, parse_ignore_string, IgnoreMap, ParseIgnoreError};
    use crate::libnxbd::nixosattributes::ConfigInfo;
    use crate::libnxbd::nixosattributes::NixUser;
    use crate::libnxbd::sshkeys::SshKeyInfo;
    use crate::libnxbd::userinfo::UserInfo;
    use crate::run_system_checks;
    use std::collections::HashMap;

    #[test]
    fn test_run_system_checks_with_ignores() {
        // Create a minimal ConfigInfo that will fail some checks
        let config_info = ConfigInfo {
            ssh_enabled: false, // This will fail ssh_enabled check
            sudo_enabled: true,
            wheel_needs_password: false,
            nix_trusts_wheel: true,
            users: vec![NixUser {
                name: "testuser".to_string(),
                ssh_keys: vec![SshKeyInfo {
                    key_type: "ssh-rsa".to_string(),
                    key_data: "AAAAB3NzaC1yc2EAAAADAQABAAABAQC".to_string(),
                    comment: "test@example.com".to_string(),
                }],
                extra_groups: vec!["wheel".to_string()],
            }],
            sudo_wheel_only: true,
            ssh_password_authentication: false,
            users_mutable: false,
            networking_firewall_enabled: true,
            log_refused_connections: false,
            boot_systemd: false,
            boot_grub: false,
            boot_systemd_generations: None,
            boot_grub_generations: None,
            nix_gc: true,
            nix_optimise_automatic: true,
            nix_auto_optimise_store: false,
            nix_extra_options: "".to_string(),
            nix_settings_experimental_features: "nix-command flakes".to_string(),
            fqdn: None,
            doc_nixos_enabled: false,
            doc_enable: false,
            doc_dev_enable: false,
            doc_doc_enable: false,
            doc_info_enable: false,
            doc_man_enable: false,
            font_fontconfig_enable: false,
            stub_ld: false,
            command_not_found: false,
            nginx_enabled: false,
            nginx_brotli: false,
            nginx_gzip: false,
            nginx_optimisation: false,
            nginx_proxy: false,
            nginx_tls: false,
            is_x86: true,
            intel_microcode: false, // This will fail cpu_microcode check
            amd_microcode: false,
            boot_is_container: false,
            host_name: "testhost".to_string(),
            system: "x86_64-linux".to_string(),
            toplevel_out: "/nix/store/test-path".to_string(),
            toplevel_drv: "/nix/store/test-drv.drv".to_string(),
            fqdn_or_host_name: "testhost".to_string(),
        };

        // Create a minimal UserInfo
        let user_info = UserInfo {
            username: "testuser".to_string(),
            ssh_keys: vec![SshKeyInfo {
                key_type: "ssh-rsa".to_string(),
                key_data: "AAAAB3NzaC1yc2EAAAADAQABAAABAQC".to_string(),
                comment: "test@example.com".to_string(),
            }],
            system: "x86_64-linux".to_string(),
            extra_platforms: vec![],
            remote_builders: vec![],
        };

        // Test 1: Without any ignores, we should have failures
        let failures = run_system_checks(&config_info, &user_info, None).unwrap();
        assert!(!failures.is_empty(), "Expected failures without ignores");

        // Verify specific failures: ssh_enabled and cpu_microcode
        assert!(
            failures.contains(&("remote_deployment".to_string(), "ssh_enabled".to_string())),
            "Expected remote_deployment.ssh_enabled to fail"
        );
        assert!(
            failures.contains(&(
                "hardware_configuration".to_string(),
                "cpu_microcode".to_string()
            )),
            "Expected hardware_configuration.cpu_microcode to fail"
        );

        // Test 2: With ignore map containing all failures, we should have no failures
        let mut ignore_map = IgnoreMap::new();

        // Add both failures to ignore map
        ignore_map.insert(
            "remote_deployment".to_string(),
            vec!["ssh_enabled".to_string()],
        );
        ignore_map.insert(
            "hardware_configuration".to_string(),
            vec!["cpu_microcode".to_string()],
        );

        let failures_with_ignores =
            run_system_checks(&config_info, &user_info, Some(&ignore_map)).unwrap();
        assert!(
            failures_with_ignores.is_empty(),
            "Expected no failures with ignores, got: {:?}",
            failures_with_ignores
        );

        // Test 3: With partial ignore map, we should have one failure
        let mut partial_ignore_map = IgnoreMap::new();

        // Only ignore ssh_enabled
        partial_ignore_map.insert(
            "remote_deployment".to_string(),
            vec!["ssh_enabled".to_string()],
        );

        let failures_with_partial_ignores =
            run_system_checks(&config_info, &user_info, Some(&partial_ignore_map)).unwrap();
        assert_eq!(
            failures_with_partial_ignores.len(),
            1,
            "Expected exactly one failure with partial ignores, got: {:?}",
            failures_with_partial_ignores
        );
        assert!(
            failures_with_partial_ignores.contains(&(
                "hardware_configuration".to_string(),
                "cpu_microcode".to_string()
            )),
            "Expected hardware_configuration.cpu_microcode to fail with partial ignores"
        );

        // Test 4: With empty vector in ignore map (should ignore ALL checks in that group)
        let mut empty_vector_ignore_map = IgnoreMap::new();

        // Use empty vector to ignore all checks in hardware_configuration
        empty_vector_ignore_map.insert("hardware_configuration".to_string(), vec![]);

        let failures_with_empty_vector =
            run_system_checks(&config_info, &user_info, Some(&empty_vector_ignore_map)).unwrap();
        assert_eq!(
            failures_with_empty_vector.len(),
            1,
            "Expected exactly one failure with empty vector in ignore map, got: {:?}",
            failures_with_empty_vector
        );
        assert!(
            failures_with_empty_vector
                .contains(&("remote_deployment".to_string(), "ssh_enabled".to_string())),
            "Expected remote_deployment.ssh_enabled to fail with empty vector in ignore map"
        );
        assert!(!failures_with_empty_vector.contains(&("hardware_configuration".to_string(), "cpu_microcode".to_string())),
            "Expected hardware_configuration.cpu_microcode to be ignored with empty vector in ignore map");
    }

    #[test]
    fn test_parse_ignore_string() {
        // Test parsing a single group and check
        let result = parse_ignore_string("group1.check1").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("group1").unwrap().len(), 1);
        assert_eq!(result.get("group1").unwrap()[0], "check1");

        // Test parsing a group with wildcard
        let result = parse_ignore_string("group2.*").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("group2").unwrap().len(), 0);

        // Test parsing multiple entries
        let result = parse_ignore_string("group1.check1,group2.*,group3.check3").unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result.get("group1").unwrap().len(), 1);
        assert_eq!(result.get("group1").unwrap()[0], "check1");
        assert_eq!(result.get("group2").unwrap().len(), 0);
        assert_eq!(result.get("group3").unwrap().len(), 1);
        assert_eq!(result.get("group3").unwrap()[0], "check3");

        // Test parsing multiple checks in the same group
        let result = parse_ignore_string("group1.check1,group1.check2").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("group1").unwrap().len(), 2);
        assert!(result
            .get("group1")
            .unwrap()
            .contains(&"check1".to_string()));
        assert!(result
            .get("group1")
            .unwrap()
            .contains(&"check2".to_string()));

        // Test parsing with spaces
        let result = parse_ignore_string(" group1.check1 , group2.* ").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result.get("group1").unwrap().len(), 1);
        assert_eq!(result.get("group2").unwrap().len(), 0);

        // Test parsing with empty input
        let result = parse_ignore_string("").unwrap();
        assert_eq!(result.len(), 0);

        // Test parsing with extra commas (should be ignored)
        let result = parse_ignore_string("group1.check1,,group2.*,").unwrap();
        assert_eq!(result.len(), 2);

        // Test error cases
        assert!(matches!(
            parse_ignore_string("invalid"),
            Err(ParseIgnoreError::NoGroup(_))
        ));
        assert!(matches!(
            parse_ignore_string(".check1"),
            Err(ParseIgnoreError::EmptyGroup(_))
        ));
        assert!(matches!(
            parse_ignore_string("group1."),
            Err(ParseIgnoreError::EmptyCheck(_))
        ));
    }

    #[test]
    fn test_merge_ignore_maps() {
        // Test case 1: Non-overlapping groups
        let map1 = parse_ignore_string("group1.check1").unwrap();
        let map2 = parse_ignore_string("group2.check2").unwrap();
        let merged = merge_ignore_maps(&map1, &map2);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged.get("group1").unwrap().len(), 1);
        assert_eq!(merged.get("group1").unwrap()[0], "check1");
        assert_eq!(merged.get("group2").unwrap().len(), 1);
        assert_eq!(merged.get("group2").unwrap()[0], "check2");

        // Test case 2: Overlapping groups with specific checks
        let map1 = parse_ignore_string("group1.check1,group2.check2").unwrap();
        let map2 = parse_ignore_string("group1.check3,group3.check4").unwrap();
        let merged = merge_ignore_maps(&map1, &map2);

        assert_eq!(merged.len(), 3);
        assert_eq!(merged.get("group1").unwrap().len(), 2);
        assert!(merged
            .get("group1")
            .unwrap()
            .contains(&"check1".to_string()));
        assert!(merged
            .get("group1")
            .unwrap()
            .contains(&"check3".to_string()));
        assert_eq!(merged.get("group2").unwrap().len(), 1);
        assert_eq!(merged.get("group2").unwrap()[0], "check2");
        assert_eq!(merged.get("group3").unwrap().len(), 1);
        assert_eq!(merged.get("group3").unwrap()[0], "check4");

        // Test case 3: Duplicate checks in the same group
        let map1 = parse_ignore_string("group1.check1,group1.check2").unwrap();
        let map2 = parse_ignore_string("group1.check2,group1.check3").unwrap();
        let merged = merge_ignore_maps(&map1, &map2);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged.get("group1").unwrap().len(), 3);
        assert!(merged
            .get("group1")
            .unwrap()
            .contains(&"check1".to_string()));
        assert!(merged
            .get("group1")
            .unwrap()
            .contains(&"check2".to_string()));
        assert!(merged
            .get("group1")
            .unwrap()
            .contains(&"check3".to_string()));

        // Test case 4: Empty vector in first map (ignore all checks in group)
        let map1 = parse_ignore_string("group1.*,group2.check1").unwrap();
        let map2 = parse_ignore_string("group1.check2,group3.check3").unwrap();
        let merged = merge_ignore_maps(&map1, &map2);

        assert_eq!(merged.len(), 3);
        assert_eq!(
            merged.get("group1").unwrap().len(),
            0,
            "Empty vector should be preserved"
        );
        assert_eq!(merged.get("group2").unwrap().len(), 1);
        assert_eq!(merged.get("group3").unwrap().len(), 1);

        // Test case 5: Empty vector in second map (ignore all checks in group)
        let map1 = parse_ignore_string("group1.check1,group2.check2").unwrap();
        let map2 = parse_ignore_string("group1.*,group3.check3").unwrap();
        let merged = merge_ignore_maps(&map1, &map2);

        assert_eq!(merged.len(), 3);
        assert_eq!(
            merged.get("group1").unwrap().len(),
            0,
            "Empty vector from map2 should be preferred"
        );
        assert_eq!(merged.get("group2").unwrap().len(), 1);
        assert_eq!(merged.get("group3").unwrap().len(), 1);

        // Test case 6: Empty maps
        let map1 = parse_ignore_string("").unwrap();
        let map2 = parse_ignore_string("").unwrap();
        let merged = merge_ignore_maps(&map1, &map2);

        assert_eq!(merged.len(), 0);

        // Test case 7: One empty map, one with content
        let map1 = parse_ignore_string("").unwrap();
        let map2 = parse_ignore_string("group1.check1").unwrap();
        let merged = merge_ignore_maps(&map1, &map2);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged.get("group1").unwrap().len(), 1);
    }
}

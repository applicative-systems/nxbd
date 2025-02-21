use super::FlakeReference;
use super::{nixosattributes::ConfigInfo, userinfo::UserInfo};
use serde_yaml;
use std::collections::HashMap;
use std::fmt;
use std::fs;

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

pub fn run_all_checks(
    config: &ConfigInfo,
    user_info: &UserInfo,
    ignored_checks: Option<&HashMap<String, HashMap<String, Vec<String>>>>,
    system: &FlakeReference,
) -> Vec<CheckGroupResult> {
    get_standard_checks()
        .iter()
        .map(|group| {
            let check_results: Vec<CheckResult> = group
                .checks
                .iter()
                .map(|check| {
                    let passed = check.check(config, user_info).is_ok();
                    let ignored = !passed
                        && ignored_checks
                            .and_then(|ic| ic.get(&system.to_string()))
                            .and_then(|system_map| system_map.get(&group.id))
                            .map(|checks| checks.contains(&check.id))
                            .unwrap_or(false);

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
                    "Set services.openssh.enable = true",
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
                    "Enable sudo in your configuration",
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
                    "Set security.sudo.wheelNeedsPassword = false",
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
                    "Add '@wheel' to nix.settings.trusted-users",
                    |config, _user_info| {
                        if !config.nix_trusts_wheel {
                            Err(CheckError {
                                check_name: "Nix Trust".to_string(),
                                message: "Wheel group is not trusted by nix".to_string(),
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
                    "Set security.sudo.execWheelOnly = true",
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
                    "log_refused_connections",
                    "The logging of refused connections should be deactivated to avoid flooding the logs and possibly leaving important messages unseen. Consider using it only for debugging firewall rules.",
                    "Set networking.firewall.logRefusedConnections = false",
                    |config, _user_info| {
                        if config.log_refused_connections {
                            Err(CheckError {
                                check_name: "Log refused connections".to_string(),
                                message: "Logging of refused connections should be disabled. Consider setting networking.firewall.logRefusedConnections = false".to_string(),
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
                    "Set boot.systemd.generations = 10 or less for systemd-boot, or boot.grub.generations = 10 or less for GRUB",
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
                    "journald_limits",
                    "journald space limits should be configured to avoid clogging the disk with logs.",
                    "Set either 'SystemKeepFree' or both 'SystemMaxUse' and 'SystemMaxFileSize'",
                    |config, _user_info| {
                        let config_str = &config.journald_extra_config;
                        let has_max_use = config_str.contains("SystemMaxUse=");
                        let has_max_file_size = config_str.contains("SystemMaxFileSize=");
                        let has_keep_free = config_str.contains("SystemKeepFree=");

                        if !has_keep_free && !(has_max_use && has_max_file_size) {
                            Err(CheckError {
                                check_name: "Journald Limits".to_string(),
                                message: "No journald space limits configured. Set either 'SystemKeepFree' or both 'SystemMaxUse' and 'SystemMaxFileSize'".to_string(),
                            })
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "nix_gc",
                    "Regular Nix Garbage Collection should be enabled",
                    "Set nix.gc.automatic = true",
                    |config, _user_info| {
                        if !config.nix_gc {
                            Err(CheckError {
                                check_name: "Garbage Collection".to_string(),
                                message: "Garbage Collection is not enabled. Consider setting nix.gc.automatic = true".to_string(),
                            })
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "nix_optimise_automatic",
                    "Nix store optimisation should be enabled",
                    "Set either nix.settings.auto-optimise-store or nix.optimise.automatic",
                    |config, _user_info| {
                        if config.boot_is_container {
                            Ok(())
                        } else if !config.nix_optimise_automatic && !config.nix_auto_optimise_store {
                            Err(CheckError {
                                check_name: "Nix store optimisation".to_string(),
                                message: "Nix store optimisation is disabled. Set either nix.settings.auto-optimise-store or nix.optimise.automatic".to_string(),
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
                    "Add 'experimental-features = nix-command flakes' to nix.extraOptions",
                    |config, _user_info| {
                        if let Some(features_line) = config.nix_extra_options
                            .lines()
                            .find(|line| line.trim().starts_with("experimental-features"))
                        {
                            if !features_line.contains("nix-command") {
                                Err(CheckError {
                                    check_name: "Nix Features".to_string(),
                                    message: "Missing required nix feature 'nix-command'. Add it to experimental-features in nix.extraOptions".to_string(),
                                })
                            } else if !features_line.contains("flakes") {
                                Err(CheckError {
                                    check_name: "Nix Features".to_string(),
                                    message: "Missing required nix feature 'flakes'. Add it to experimental-features in nix.extraOptions".to_string(),
                                })
                            } else {
                                Ok(())
                            }
                        } else {
                            Err(CheckError {
                                check_name: "Nix Features".to_string(),
                                message: "No experimental-features configured. Add 'experimental-features = nix-command flakes' to nix.extraOptions".to_string(),
                            })
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
                    "Set documentation.nixos.enable = false",
                    |config, _user_info| {
                        if config.fqdn.is_some() {
                            if config.doc_nixos_enabled {
                                Err(CheckError {
                                    check_name: "Documentation".to_string(),
                                    message: "NixOS documentation enabled. Consider setting documentation.nixos.enable = false".to_string(),
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
                    "Set documentation.enable = false",
                    |config, _user_info| {
                        if config.fqdn.is_some() {
                            if config.doc_enable {
                                Err(CheckError {
                                    check_name: "Documentation".to_string(),
                                    message: "General documentation enabled. Consider setting documentation.enable = false".to_string(),
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
                    "Set documentation.dev.enable = false",
                    |config, _user_info| {
                        if config.fqdn.is_some() {
                            if config.doc_dev_enable {
                                Err(CheckError {
                                    check_name: "Documentation".to_string(),
                                    message: "Development documentation enabled. Consider setting documentation.dev.enable = false".to_string(),
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
                    "Set documentation.doc.enable = false",
                    |config, _user_info| {
                        if config.fqdn.is_some() {
                            if config.doc_doc_enable {
                                Err(CheckError {
                                    check_name: "Documentation".to_string(),
                                    message: "Doc documentation enabled. Consider setting documentation.doc.enable = false".to_string(),
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
                    "Set documentation.info.enable = false",
                    |config, _user_info| {
                        if config.fqdn.is_some() {
                            if config.doc_info_enable {
                                Err(CheckError {
                                    check_name: "Documentation".to_string(),
                                    message: "Info documentation enabled. Consider setting documentation.info.enable = false".to_string(),
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
                    "Set documentation.man.enable = false",
                    |config, _user_info| {
                        if config.fqdn.is_some() {
                            if config.doc_man_enable {
                                Err(CheckError {
                                    check_name: "Documentation".to_string(),
                                    message: "Man pages enabled. Consider setting documentation.man.enable = false".to_string(),
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
                    "nginx_brotli",
                    "Brotli compression should be enabled",
                    "Set services.nginx.recommendedBrotliSettings = true",
                    |config, _user_info| {
                        if config.nginx_enabled {
                            if !config.nginx_brotli {
                                Err(CheckError {
                                    check_name: "Nginx Settings".to_string(),
                                    message: "Brotli compression not enabled. Consider setting services.nginx.recommendedBrotliSettings = true".to_string(),
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
                    "Set services.nginx.recommendedGzipSettings = true",
                    |config, _user_info| {
                        if config.nginx_enabled {
                            if !config.nginx_gzip {
                                Err(CheckError {
                                    check_name: "Nginx Settings".to_string(),
                                    message: "Gzip compression not enabled. Consider setting services.nginx.recommendedGzipSettings = true".to_string(),
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
                    "Set services.nginx.recommendedOptimisation = true",
                    |config, _user_info| {
                        if config.nginx_enabled {
                            if !config.nginx_optimisation {
                                Err(CheckError {
                                    check_name: "Nginx Settings".to_string(),
                                    message: "Optimisation settings not enabled. Consider setting services.nginx.recommendedOptimisation = true".to_string(),
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
                    "Set services.nginx.recommendedProxySettings = true",
                    |config, _user_info| {
                        if config.nginx_enabled {
                            if !config.nginx_proxy {
                                Err(CheckError {
                                    check_name: "Nginx Settings".to_string(),
                                    message: "Proxy settings not enabled. Consider setting services.nginx.recommendedProxySettings = true".to_string(),
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
                    "Set services.nginx.recommendedTlsSettings = true",
                    |config, _user_info| {
                        if config.nginx_enabled {
                            if !config.nginx_tls {
                                Err(CheckError {
                                    check_name: "Nginx Settings".to_string(),
                                    message: "TLS settings not enabled. Consider setting services.nginx.recommendedTlsSettings = true".to_string(),
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
                    "Set either hardware.cpu.intel.updateMicrocode or hardware.cpu.amd.updateMicrocode",
                    |config, _user_info| {
                        if config.is_x86 {
                            if !config.intel_microcode && !config.amd_microcode {
                                Err(CheckError {
                                    check_name: "Microcode".to_string(),
                                    message: "No CPU microcode updates enabled. Set either hardware.cpu.intel.updateMicrocode or hardware.cpu.amd.updateMicrocode to true".to_string(),
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
        let mut system_map: HashMap<String, Vec<String>> = HashMap::new();

        for group in results {
            let failed_checks: Vec<String> = group
                .checks
                .iter()
                .filter(|check| !check.passed)
                .map(|check| check.id.clone())
                .collect();

            if !failed_checks.is_empty() {
                system_map.insert(group.id.clone(), failed_checks);
            }
        }

        if !system_map.is_empty() {
            // Replace or insert the system's ignored checks
            ignore_map.insert(system.to_string(), system_map);
        } else {
            // If no failures for this system, remove it from ignored checks
            ignore_map.remove(&system.to_string());
        }
    }

    if !ignore_map.is_empty() {
        let yaml = serde_yaml::to_string(&ignore_map)?;
        fs::write(path, yaml)?;
    }

    Ok(())
}

pub fn load_ignored_checks(path: &str) -> Option<HashMap<String, HashMap<String, Vec<String>>>> {
    match fs::read_to_string(path) {
        Ok(contents) => serde_yaml::from_str(&contents).ok(),
        Err(_) => None,
    }
}

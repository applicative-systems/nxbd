use super::{nixosattributes::ConfigInfo, userinfo::UserInfo};
use std::fmt;

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

impl CheckGroup {
    pub fn run_checks(
        &self,
        config: &ConfigInfo,
        user_info: &UserInfo,
    ) -> (bool, Vec<(String, bool)>) {
        let check_results: Vec<(String, bool)> = self
            .checks
            .iter()
            .map(|check| (check.id.clone(), check.check(config, user_info).is_ok()))
            .collect();

        let group_passed = check_results.iter().all(|(_, passed)| *passed);
        (group_passed.clone(), check_results)
    }
}

pub fn run_all_checks(
    config: &ConfigInfo,
    user_info: &UserInfo,
) -> Vec<(String, bool, Vec<(String, bool)>)> {
    get_standard_checks()
        .iter()
        .map(|group| {
            let (group_passed, check_results) = group.run_checks(config, user_info);
            (group.id.clone(), group_passed.clone(), check_results)
        })
        .collect()
}

pub fn get_standard_checks() -> Vec<CheckGroup> {
    vec![
        CheckGroup {
            id: "remote_deployment".to_string(),
            name: "Remote Deployment Support".to_string(),
            description: "Checks if the system has the required configuration to safely perform remote deployments".to_string(),
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
            ],
        },
        CheckGroup {
            id: "sudo_security".to_string(),
            name: "Sudo Security Settings".to_string(),
            description: "Checks if sudo is configured securely".to_string(),
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
            ],
        },
        CheckGroup {
            id: "firewall_settings".to_string(),
            name: "Firewall settings".to_string(),
            description: "Check whether firewall is configured correctly".to_string(),
            checks: vec![
                Check::new(
                    "log_refused_connections",
                    "Logging of refused connections should be disabled",
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
            id: "boot_configuration_limit".to_string(),
            name: "Boot Configuration Limit".to_string(),
            description: "Checks if system configuration generations are reasonably limited to prevent disk space waste".to_string(),
            checks: vec![
                Check::new(
                    "boot_systemd_generations",
                    "systemd-boot generations should be limited",
                    "Set boot.systemd.generations = 10 or less",
                    |config, _user_info| {
                        if config.boot_systemd {
                            if let Some(limit) = config.boot_systemd_generations {
                                if limit > 10 {
                                    Err(CheckError {
                                        check_name: "systemd-boot Generations".to_string(),
                                        message: format!(
                                            "Too many generations kept ({}). Consider reducing to 10 or less",
                                            limit
                                        ),
                                    })
                                } else {
                                    Ok(())
                                }
                            } else {
                                Err(CheckError {
                                    check_name: "systemd-boot Generations".to_string(),
                                    message: "No generation limit set. This may prevent old generations from being garbage collected".to_string(),
                                })
                            }
                        } else {
                            Ok(())
                        }
                    },
                ),
                Check::new(
                    "boot_grub_generations",
                    "GRUB generations should be limited",
                    "Set boot.grub.generations = 10 or less",
                    |config, _user_info| {
                        if config.boot_grub {
                            if let Some(limit) = config.boot_grub_generations {
                                if limit > 10 {
                                    Err(CheckError {
                                        check_name: "GRUB Generations".to_string(),
                                        message: format!(
                                            "Too many generations kept ({}). Consider reducing to 10 or less",
                                            limit
                                        ),
                                    })
                                } else {
                                    Ok(())
                                }
                            } else {
                                Err(CheckError {
                                    check_name: "GRUB Generations".to_string(),
                                    message: "No generation limit set. This may prevent old generations from being garbage collected".to_string(),
                                })
                            }
                        } else {
                            Ok(())
                        }
                    },
                ),
            ],
        },
        CheckGroup {
            id: "disk_space_management".to_string(),
            name: "Disk Space Management".to_string(),
            description: "Checks whether the optimisations and limits for disk space are configured".to_string(),
            checks: vec![
                Check::new(
                    "journald_limits",
                    "journald space limits should be configured",
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
                    "nix_optimise_automatic",
                    "Nix store optimisation should be enabled",
                    "Set either nix.settings.auto-optimise-store or nix.optimise.automatic",
                    |config, _user_info| {
                        if !config.nix_optimise_automatic && !config.nix_auto_optimise_store {
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
            id: "nix_flakes".to_string(),
            name: "Nix Flakes".to_string(),
            description: "Checks if flakes are enabled".to_string(),
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
            id: "disable_documentation".to_string(),
            name: "Disable Documentation on Servers".to_string(),
            description: "Checks if documentation is disabled on servers to reduce closure size".to_string(),
            checks: vec![
                Check::new(
                    "doc_nixos_enabled",
                    "NixOS documentation should be disabled",
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
                    "doc_enable",
                    "General documentation should be disabled",
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
                    "doc_dev_enable",
                    "Development documentation should be disabled",
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
                    "doc_doc_enable",
                    "Doc documentation should be disabled",
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
                    "doc_info_enable",
                    "Info documentation should be disabled",
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
                    "doc_man_enable",
                    "Man pages should be disabled",
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
            ],
        },
        CheckGroup {
            id: "enable_cpu_microcode_updates".to_string(),
            name: "Enable CPU Microcode Updates on x86".to_string(),
            description: "Checks if CPU microcode updates are enabled on x86 systems".to_string(),
            checks: vec![
                Check::new(
                    "cpu_microcode",
                    "CPU microcode updates should be enabled",
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
        CheckGroup {
            id: "nginx_recommended_settings".to_string(),
            name: "Nginx Recommended Settings".to_string(),
            description: "Checks if nginx has recommended settings enabled".to_string(),
            checks: vec![
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
            id: "garbage_collection".to_string(),
            name: "Garbage Collection".to_string(),
            description: "Checks whether the Nix garbage collection is configured correctly".to_string(),
            checks: vec![
                Check::new(
                    "nix_gc",
                    "Garbage Collection should be enabled",
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
            ],
        },
    ]
}

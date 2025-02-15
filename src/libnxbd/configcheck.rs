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

#[allow(clippy::module_name_repetitions)]
pub struct ConfigCheck {
    pub name: String,
    pub description: String,
    check_fn: Box<dyn Fn(&ConfigInfo, &UserInfo) -> Result<(), Vec<CheckError>>>,
}

impl ConfigCheck {
    pub fn new<F>(name: &str, description: &str, check_fn: F) -> Self
    where
        F: Fn(&ConfigInfo, &UserInfo) -> Result<(), Vec<CheckError>> + 'static,
    {
        ConfigCheck {
            name: name.to_string(),
            description: description.to_string(),
            check_fn: Box::new(check_fn),
        }
    }

    pub fn check(&self, config: &ConfigInfo, user_info: &UserInfo) -> Result<(), Vec<CheckError>> {
        (self.check_fn)(config, user_info)
    }
}

pub fn get_standard_checks() -> Vec<ConfigCheck> {
    vec![
        ConfigCheck::new(
            "Remote Deployment Support",
            "Checks if the system has the required configuration (SSH, sudo, permissions) to safely perform remote deployments",
            |config, user_info| {
                let mut errors = Vec::new();

                if !config.ssh_enabled {
                    errors.push(CheckError {
                        check_name: "SSH".to_string(),
                        message: "SSH service is not enabled".to_string(),
                    });
                }

                if !config.sudo_enabled {
                    errors.push(CheckError {
                        check_name: "Sudo".to_string(),
                        message: "Sudo is not enabled".to_string(),
                    });
                }

                if config.wheel_needs_password {
                    errors.push(CheckError {
                        check_name: "Sudo Password".to_string(),
                        message: "Wheel group members need password for sudo".to_string(),
                    });
                }

                if !config.nix_trusts_wheel {
                    errors.push(CheckError {
                        check_name: "Nix Trust".to_string(),
                        message: "Wheel group is not trusted by nix (add '@wheel' to nix.settings.trusted-users)".to_string(),
                    });
                }

                // Check if current user has SSH access
                let current_user = &user_info.username;
                let user = config.users.iter().find(|u| u.name == *current_user);

                match user {
                    None => {
                        errors.push(CheckError {
                            check_name: "User Access".to_string(),
                            message: format!(
                                "User '{}' does not exist on target system",
                                current_user
                            ),
                        });
                    }
                    Some(user) => {
                        let has_matching_key = user_info
                            .ssh_keys
                            .iter()
                            .any(|local_key| user.ssh_keys.contains(&local_key));

                        if !has_matching_key {
                            errors.push(CheckError {
                                check_name: "User Access".to_string(),
                                message: format!(
                                    "User '{}' exists but none of their local SSH keys are authorized on target system",
                                    current_user
                                ),
                            });
                        }
                    }
                }

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            },
        ),
        ConfigCheck::new(
            "Sudo Security Settings",
            "Check if sudo is configured securely",
            |config, _user_info| {
                let mut errors = Vec::new();

                if !config.sudo_wheel_only {
                    errors.push(CheckError {
                        check_name: "Sudo Wheel Only".to_string(),
                        message: "Only users of the wheel group should be allowed to use sudo. Consider setting security.sudo.execWheelOnly".to_string(),
                    });
                }

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            },
        ),
        ConfigCheck::new(
            "Boot Configuration Limit",
            "Checks if system configuration generations are reasonably limited to prevent disk space waste",
            |config, _user_info| {
                let mut errors = Vec::new();

                if config.boot_systemd {
                    if let Some(limit) = config.boot_systemd_generations {
                        if limit > 10 {
                            errors.push(CheckError {
                                check_name: "systemd-boot Generations".to_string(),
                                message: format!(
                                    "Too many generations kept ({}). Consider reducing to 10 or less",
                                    limit
                                ),
                            });
                        }
                    } else {
                        errors.push(CheckError {
                            check_name: "systemd-boot Generations".to_string(),
                            message: "No generation limit set. This may prevent old generations from being garbage collected".to_string(),
                        });
                    }
                }

                if config.boot_grub {
                    if let Some(limit) = config.boot_grub_generations {
                        if limit > 10 {
                            errors.push(CheckError {
                                check_name: "GRUB Generations".to_string(),
                                message: format!(
                                    "Too many generations kept ({}). Consider reducing to 10 or less",
                                    limit
                                ),
                            });
                        }
                    } else {
                        errors.push(CheckError {
                            check_name: "GRUB Generations".to_string(),
                            message: "No generation limit set. This may prevent old generations from being garbage collected".to_string(),
                        });
                    }
                }

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            },
        ),
        ConfigCheck::new(
            "Journald Space Management",
            "Checks if journald has proper disk space limits configured",
            |config, _user_info| {
                let mut errors = Vec::new();
                let config_str = &config.journald_extra_config;

                let has_max_use = config_str.contains("SystemMaxUse=");
                let has_max_file_size = config_str.contains("SystemMaxFileSize=");
                let has_keep_free = config_str.contains("SystemKeepFree=");

                if !has_keep_free && !(has_max_use && has_max_file_size) {
                    errors.push(CheckError {
                        check_name: "Journald Limits".to_string(),
                        message: "No journald space limits configured. Set either 'SystemKeepFree' or both 'SystemMaxUse' and 'SystemMaxFileSize'".to_string(),
                    });
                }

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            },
        ),
        ConfigCheck::new(
            "Nix Flakes",
            "Checks if flakes are enabled",
            |config, _user_info| {
                let mut errors = Vec::new();
                if let Some(features_line) = config.nix_extra_options
                    .lines()
                    .find(|line| line.trim().starts_with("experimental-features"))
                {
                    if !features_line.contains("nix-command") {
                        errors.push(CheckError {
                            check_name: "Nix Features".to_string(),
                            message: "Missing required nix feature 'nix-command'. Add it to experimental-features in nix.extraOptions".to_string(),
                        });
                    }
                    if !features_line.contains("flakes") {
                        errors.push(CheckError {
                            check_name: "Nix Features".to_string(),
                            message: "Missing required nix feature 'flakes'. Add it to experimental-features in nix.extraOptions".to_string(),
                        });
                    }
                } else {
                    errors.push(CheckError {
                        check_name: "Nix Features".to_string(),
                        message: "No experimental-features configured. Add 'experimental-features = nix-command flakes' to nix.extraOptions".to_string(),
                    });
                }

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            },
        ),
        ConfigCheck::new(
            "Disable Documentation on Servers",
            "Checks if documentation is disabled on servers to reduce closure size",
            |config, _user_info| {
                let mut errors = Vec::new();
                // Only check servers (those with FQDN set)
                if config.fqdn.is_some() {
                    if config.doc_nixos_enabled {
                        errors.push(CheckError {
                            check_name: "Documentation".to_string(),
                            message: "NixOS documentation enabled. Consider setting documentation.nixos.enable = false".to_string(),
                        });
                    }
                    if config.doc_enable {
                        errors.push(CheckError {
                            check_name: "Documentation".to_string(),
                            message: "General documentation enabled. Consider setting documentation.enable = false".to_string(),
                        });
                    }
                    if config.doc_dev_enable {
                        errors.push(CheckError {
                            check_name: "Documentation".to_string(),
                            message: "Development documentation enabled. Consider setting documentation.dev.enable = false".to_string(),
                        });
                    }
                    if config.doc_doc_enable {
                        errors.push(CheckError {
                            check_name: "Documentation".to_string(),
                            message: "Doc documentation enabled. Consider setting documentation.doc.enable = false".to_string(),
                        });
                    }
                    if config.doc_info_enable {
                        errors.push(CheckError {
                            check_name: "Documentation".to_string(),
                            message: "Info documentation enabled. Consider setting documentation.info.enable = false".to_string(),
                        });
                    }
                    if config.doc_man_enable {
                        errors.push(CheckError {
                            check_name: "Documentation".to_string(),
                            message: "Man pages enabled. Consider setting documentation.man.enable = false".to_string(),
                        });
                    }
                }

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            },
        ),
        ConfigCheck::new(
            "Enable CPU Microcode Updates on x86",
            "Checks if CPU microcode updates are enabled on x86 systems",
            |config, _user_info| {
                let mut errors = Vec::new();
                if config.is_x86 {
                    if !config.intel_microcode && !config.amd_microcode {
                        errors.push(CheckError {
                            check_name: "Microcode".to_string(),
                            message: "No CPU microcode updates enabled. Set either hardware.cpu.intel.updateMicrocode or hardware.cpu.amd.updateMicrocode to true".to_string(),
                        });
                    }
                }

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            }
        ),
        ConfigCheck::new(
            "Nginx Recommended Settings",
            "Checks if nginx has recommended settings enabled",
            |config, _user_info| {
                let mut errors = Vec::new();
                if config.nginx_enabled {
                    if !config.nginx_brotli {
                        errors.push(CheckError {
                            check_name: "Nginx Settings".to_string(),
                            message: "Brotli compression not enabled. Consider setting services.nginx.recommendedBrotliSettings = true".to_string(),
                        });
                    }
                    if !config.nginx_gzip {
                        errors.push(CheckError {
                            check_name: "Nginx Settings".to_string(),
                            message: "Gzip compression not enabled. Consider setting services.nginx.recommendedGzipSettings = true".to_string(),
                        });
                    }
                    if !config.nginx_optimisation {
                        errors.push(CheckError {
                            check_name: "Nginx Settings".to_string(),
                            message: "Optimisation settings not enabled. Consider setting services.nginx.recommendedOptimisation = true".to_string(),
                        });
                    }
                    if !config.nginx_proxy {
                        errors.push(CheckError {
                            check_name: "Nginx Settings".to_string(),
                            message: "Proxy settings not enabled. Consider setting services.nginx.recommendedProxySettings = true".to_string(),
                        });
                    }
                    if !config.nginx_tls {
                        errors.push(CheckError {
                            check_name: "Nginx Settings".to_string(),
                            message: "TLS settings not enabled. Consider setting services.nginx.recommendedTlsSettings = true".to_string(),
                        });
                    }
                }

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            },
        ),
        ConfigCheck::new(
            "Garbage Collection",
            "Checks whether the Nix garbage collection is configured correctly",
            |config, _user_info| {
                let mut errors = Vec::new();
                if !config.nix_gc {
                    errors.push(CheckError {
                        check_name: "Garbage Collection".to_string(),
                        message: "Garbage Collection is not enabled. Consider setting nix.gc.automatic = true".to_string(),
                    });
                }

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            },
        ),
    ]
}

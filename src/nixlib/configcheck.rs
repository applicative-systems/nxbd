use super::{deployinfo::ConfigInfo, userinfo::UserInfo};
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
    ]
}

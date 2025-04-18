use super::nixcommands::{get_remote_builders, get_system, NixError, RemoteBuilder};
use super::sshkeys::SshKeyInfo;
use std::env;
use std::process::Command;

#[derive(Debug)]
pub struct UserInfo {
    pub username: String,
    pub ssh_keys: Vec<SshKeyInfo>,
    pub system: String,
    pub extra_platforms: Vec<String>,
    pub remote_builders: Vec<RemoteBuilder>,
}

impl UserInfo {
    pub fn collect() -> Result<Self, NixError> {
        let username = env::var("USER")
            .map_err(|_| NixError::Eval("Failed to get username from environment".to_string()))?;

        let ssh_keys = Command::new("ssh-add")
            .arg("-L")
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|output| {
                output
                    .lines()
                    .filter_map(SshKeyInfo::from_authorized_key)
                    .collect()
            })
            .unwrap_or_else(Vec::new);

        let (system, extra_platforms) = get_system()?;
        let remote_builders = get_remote_builders()?;

        Ok(UserInfo {
            username,
            ssh_keys,
            system,
            extra_platforms,
            remote_builders,
        })
    }

    pub fn can_build_natively(&self, target_system: &str) -> bool {
        // Can build natively if:
        // 1. Target system matches user's system, or
        // 2. Target system is in extra-platforms, or
        // 3. User has a remote builder configured for the target system
        target_system == self.system
            || self.extra_platforms.contains(&target_system.to_string())
            || self
                .remote_builders
                .iter()
                .any(|rb| rb.system == target_system)
    }
}

use super::sshkeys::SshKeyInfo;
use std::process::Command;

#[derive(Debug)]
pub struct UserInfo {
    pub username: String,
    pub ssh_keys: Vec<SshKeyInfo>,
}

impl UserInfo {
    pub fn collect() -> Self {
        let username = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());

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

        UserInfo { username, ssh_keys }
    }
}

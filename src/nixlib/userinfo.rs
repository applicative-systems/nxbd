use std::path::PathBuf;
use std::fs;
use std::process::Command;

#[derive(Debug)]
pub struct SshKeyInfo {
    pub comment: String,
    pub key_type: String,
    pub key_data: String,
}

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
                    .filter_map(|line| {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        match parts.as_slice() {
                            [key_type, key_data, comment, ..] => Some(SshKeyInfo {
                                key_type: key_type.to_string(),
                                key_data: key_data.to_string(),
                                comment: comment.to_string(),
                            }),
                            [key_type, key_data] => Some(SshKeyInfo {
                                key_type: key_type.to_string(),
                                key_data: key_data.to_string(),
                                comment: String::new(),
                            }),
                            _ => None,
                        }
                    })
                    .collect()
            })
            .unwrap_or_else(Vec::new);

        UserInfo {
            username,
            ssh_keys,
        }
    }
} 
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKeyInfo {
    pub comment: String,
    pub key_type: String,
    pub key_data: String,
}

impl PartialEq for SshKeyInfo {
    fn eq(&self, other: &Self) -> bool {
        self.key_type == other.key_type && self.key_data == other.key_data
    }
}

impl Eq for SshKeyInfo {}

impl SshKeyInfo {
    pub fn from_authorized_key(key_string: &str) -> Option<Self> {
        let parts: Vec<&str> = key_string.split_whitespace().collect();
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
    }
}

impl fmt::Display for SshKeyInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.comment.is_empty() {
            write!(f, "{} {}", self.key_type, self.key_data)
        } else {
            write!(f, "{} {} {}", self.key_type, self.key_data, self.comment)
        }
    }
}

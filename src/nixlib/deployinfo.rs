use super::{FlakeReference, NixError};

use serde::Deserialize;
use std::process;
use std::str;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // the deserialization code actually touches all fields
pub struct ConfigInfo {
    // The machine's host name
    pub hostname: Option<String>,
    // The machine's fully qualified domain name
    pub fqdn: Option<String>,
    // The machine's fully qualified domain name or host name
    pub fqdn_or_host_name: Option<String>,
    // Whether the wheel user needs a password to sudo
    pub wheel_needs_password: bool,
    // Whether SSH is enabled
    pub ssh_enabled: bool,
    // Whether sudo is enabled
    pub sudo_enabled: bool,
    // Whether the nix user trusts the wheel group
    pub nix_trusts_wheel: bool,
    // Users with their SSH keys
    pub users: Vec<NixUser>,
}

#[derive(Deserialize, Debug)]
struct NixUser {
    pub name: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub extraGroups: Vec<String>,
    #[serde(default)]
    pub sshKeys: Vec<String>,
}

pub fn nixos_deploy_info(flake_reference: &FlakeReference) -> Result<ConfigInfo, NixError> {
    let nix_expr = r#"config:
          let
            f = expr: let x = builtins.tryEval expr; in if x.success then x.value else null;
            normalUsers = builtins.filter
                (user: (user.isNormalUser or false))
                (builtins.attrValues config.users.users);
          in
            {
                hostname = f config.networking.hostName;
                fqdn = f config.networking.fqdn;
                fqdnOrHostName = f config.networking.fqdnOrHostName;
                wheelNeedsPassword = config.security.sudo.wheelNeedsPassword;
                sshEnabled = config.services.openssh.enable;
                sudoEnabled = config.security.sudo.enable;
                nixTrustsWheel = builtins.elem "@wheel" config.nix.settings.trusted-users;
                users = map (user: {
                    name = user.name;
                    group = user.group or "";
                    extraGroups = user.extraGroups or [];
                    sshKeys = user.openssh.authorizedKeys.keys or [];
                }) normalUsers;
            }"#;

    let output = std::process::Command::new("nix")
        .args([
            "eval",
            "--json",
            &format!(
                "{}#nixosConfigurations.\"{}\".config",
                flake_reference.url, flake_reference.attribute
            ),
            "--apply",
            nix_expr,
        ])
        .output()
        .map_err(|_| NixError::Eval)?;

    let stdout_str = str::from_utf8(&output.stdout).expect("Failed to convert to string");

    serde_json::from_str(&stdout_str).map_err(|_| NixError::Deserialization)
}

impl ConfigInfo {
    pub fn get_users_with_ssh_keys(
        &self,
        _flake_reference: &FlakeReference,
    ) -> Result<Vec<(String, bool, Vec<String>)>, NixError> {
        Ok(self
            .users
            .iter()
            .filter(|user| !user.sshKeys.is_empty())
            .map(|user| {
                (
                    user.name.clone(),
                    user.extraGroups.contains(&"wheel".to_string()),
                    user.sshKeys.clone(),
                )
            })
            .collect())
    }
}

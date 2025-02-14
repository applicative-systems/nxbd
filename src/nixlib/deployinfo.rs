use super::sshkeys::SshKeyInfo;
use super::{FlakeReference, NixError};

use serde::Deserialize;
use std::str;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::module_name_repetitions)]
pub struct ConfigInfo {
    pub fqdn_or_host_name: Option<String>,
    pub wheel_needs_password: bool,
    pub ssh_enabled: bool,
    pub sudo_enabled: bool,
    pub nix_trusts_wheel: bool,
    pub boot_systemd: bool,
    pub boot_grub: bool,
    pub boot_systemd_generations: Option<i32>,
    pub boot_grub_generations: Option<i32>,
    pub journald_extra_config: String,
    pub nix_extra_options: String,
    pub users: Vec<NixUser>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::module_name_repetitions)]
pub struct NixUser {
    pub name: String,
    pub extra_groups: Vec<String>,
    #[serde(deserialize_with = "deserialize_ssh_keys")]
    pub ssh_keys: Vec<SshKeyInfo>,
}

fn deserialize_ssh_keys<'de, D>(deserializer: D) -> Result<Vec<SshKeyInfo>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let strings: Vec<String> = Vec::deserialize(deserializer)?;
    Ok(strings
        .iter()
        .filter_map(|s| SshKeyInfo::from_authorized_key(s))
        .collect())
}

pub fn nixos_deploy_info(flake_reference: &FlakeReference) -> Result<ConfigInfo, NixError> {
    // At this point we're just mindlessly piling up all the attributes of a
    // config that the checks would ever need. Maybe at some point in the future
    // this should be modularized.
    let nix_expr = r#"config:
          let
            f = expr: let x = builtins.tryEval expr; in if x.success then x.value else null;
            normalUsers = builtins.filter
                (user: (user.isNormalUser or false))
                (builtins.attrValues config.users.users);
          in
            {
                fqdnOrHostName = f config.networking.fqdnOrHostName;
                wheelNeedsPassword = config.security.sudo.wheelNeedsPassword;
                sshEnabled = config.services.openssh.enable;
                sudoEnabled = config.security.sudo.enable;
                nixTrustsWheel = builtins.elem "@wheel" config.nix.settings.trusted-users;
                bootSystemd = config.boot.loader.systemd-boot.enable;
                bootGrub = config.boot.loader.grub.enable;
                bootSystemdGenerations = f config.boot.loader.systemd-boot.configurationLimit;
                bootGrubGenerations = f config.boot.loader.grub.configurationLimit;
                journaldExtraConfig = config.services.journald.extraConfig;
                nixExtraOptions = config.nix.extraOptions;
                users = map (user: {
                    name = user.name;
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
        .map_err(|_| NixError::Eval("Failed to execute nix eval".to_string()))?;

    if !output.status.success() {
        return Err(NixError::Eval(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }

    let stdout_str = str::from_utf8(&output.stdout).map_err(|_| NixError::Deserialization)?;

    serde_json::from_str(&stdout_str).map_err(|_| NixError::Deserialization)
}

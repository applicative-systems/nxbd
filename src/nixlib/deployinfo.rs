use super::{FlakeReference, NixError};

use serde::Deserialize;
use std::process;
use std::str;

//TODO try #[serde(rename_all = "snake_case")]

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
    pub nix_trusts_wheel: bool
}

fn config_info_nix_expression(system_name: &str, flake: &str) -> String {
    format!(
        r#"let
  flake = builtins.getFlake "{flake}";
  systemName = "{system_name}";
  inherit (flake.nixosConfigurations.${{systemName}}) config;
  f = expr: let x = builtins.tryEval expr; in if x.success then x.value else null;
in
{{
  hostname = f config.networking.hostName;
  fqdn = f config.networking.fqdn;
  fqdnOrHostName = f config.networking.fqdnOrHostName;
  wheelNeedsPassword = config.security.sudo.wheelNeedsPassword;
  sshEnabled = config.services.openssh.enable;
  sudoEnabled = config.security.sudo.enable;
  nixTrustsWheel = builtins.elem "@wheel" config.nix.settings.trusted-users;
}}"#,
        flake = flake,
        system_name = system_name
    )
}

fn config_info_sshkeys(system_name: &str, flake: &str, user: &str) -> String {
    format!(
        r#"let
  flake = builtins.getFlake "{flake}";
  systemName = "{system_name}";
  user = "{user}";
  inherit (flake.nixosConfigurations.${{systemName}}) config;
  f = alt: expr: let x = builtins.tryEval expr; in if x.success then x.value else alt;
in
f [] config.users.users.${{user}}.openssh.authorizedKeys.keys
"#,
        flake = flake,
        system_name = system_name,
        user = user
    )
}

pub fn nixos_deploy_info(flake_reference: &FlakeReference) -> Result<ConfigInfo, NixError> {
    let build_output: process::Output = process::Command::new("nix")
        .args([
            "eval",
            "--impure",
            "--json",
            "--expr",
            &config_info_nix_expression(&flake_reference.attribute, &flake_reference.url),
        ])
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|_| NixError::Eval)?;

    let stdout_str = str::from_utf8(&build_output.stdout).expect("Failed to convert to string");
    //println!("stdout_str = {:?}", stdout_str);

    let deserialized: ConfigInfo = serde_json::from_str(&stdout_str).unwrap();
    Ok(deserialized)
}

pub fn nixos_deploy_ssh_keys(flake_reference: &FlakeReference, user: &str) -> Result<Vec<String>, NixError> {
    let build_output: process::Output = process::Command::new("nix")
        .args([
            "eval",
            "--impure",
            "--json",
            "--expr",
            &config_info_sshkeys(&flake_reference.attribute, &flake_reference.url, &user),
        ])
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|_| NixError::Eval)?;

    let stdout_str = str::from_utf8(&build_output.stdout).expect("Failed to convert to string");

    let deserialized: Vec<String> = serde_json::from_str(&stdout_str).unwrap();
    //println!("deserialized = {:?}", deserialized);
    Ok(deserialized)
}
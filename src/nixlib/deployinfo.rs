use super::{FlakeReference, NixError};

use std::process;
use std::str;

#[derive(Debug)]
pub struct DeployInfo {
    pub target_hostname: String,
    pub target_username: Option<String>,
    pub remote_private_keyfile: Option<String>,
    pub remote_sudo: bool,
}

fn nixos_fqdn(flake_reference: &FlakeReference) -> Result<String, NixError> {
    let build_output: process::Output = process::Command::new("nix")
        .args([
            "eval",
            "--raw",
            &format!("{0}#nixosConfigurations.\"{1}\".config.networking.fqdn", flake_reference.url, flake_reference.attribute),
        ])
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|_| NixError::Eval)?;

    let stdout_str = str::from_utf8(&build_output.stdout).expect("Failed to convert to string");
    Ok(stdout_str.to_string())
}

pub fn acquire_deploy_info(flake_ref: &FlakeReference) -> Result<DeployInfo, NixError> {
    let fqdn = nixos_fqdn(flake_ref)?;

    Ok(DeployInfo{
        target_hostname: fqdn,
        target_username: None,
        remote_private_keyfile: None,
        remote_sudo: true,
    })
}
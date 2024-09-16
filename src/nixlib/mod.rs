pub mod flakeref;
mod outputhandling;

use std::process;
use std::str;

pub use flakeref::FlakeReference;

#[derive(Debug)]
pub enum NixError {
    Eval,
    Build,
    ConfigSwitch,
    ProfileSet,
    Deserialization,
}

pub fn nixos_configuration_attributes(flake_url: &str) -> Result<Vec<String>, NixError> {
    let build_output = process::Command::new("nix")
        .args([
            "eval",
            "--json",
            &format!("{flake_url}#nixosConfigurations"),
            "--apply",
            "builtins.attrNames"
        ])
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|_| NixError::Eval)?;

    let stdout_str = str::from_utf8(&build_output.stdout).expect("Failed to convert to string");
    let attributes: Vec<String> = serde_json::from_str(stdout_str)
        .map_err(|_| NixError::Deserialization)?;

    Ok(attributes)
}

pub fn nixos_configuration_flakerefs(flake_url: &str) -> Result<Vec<FlakeReference>, NixError> {
    let discovered_attrs = nixos_configuration_attributes(flake_url)?;
    let flakerefs = discovered_attrs
        .into_iter()
        .map(|x| FlakeReference{ 
            url: flake_url.to_string(), 
            attribute: x 
        })
        .collect();
    Ok(flakerefs)
}

pub fn nixos_fqdn(flake_reference: &FlakeReference) -> Result<String, NixError> {
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



pub fn toplevel_output_path(flake_reference: &FlakeReference) -> Result<String, NixError> {
    let build_output = process::Command::new("nom")
        .args([
            "build",
            "--json",
            &format!("{0}#nixosConfigurations.\"{1}\".config.system.build.toplevel", flake_reference.url, flake_reference.attribute),
        ])
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|_| NixError::Build)?;

    let output_path = outputhandling::single_nix_build_output(&build_output.stdout)
        .map_err(|_| NixError::Deserialization)?;
    Ok(output_path)
}

pub fn activate_profile(toplevel_path: &str) -> Result<(), NixError> {
    process::Command::new("sudo")
        .args([
            "nix-env",
            "-p",
            "/nix/var/nix/profiles/system",
            "--set",
            toplevel_path
        ])
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|_| NixError::ProfileSet)
        .map(|_| ())
}

pub fn switch_to_configuration(toplevel_path: &str, command: &str) -> Result<(),NixError> {
    process::Command::new("sudo")
        .args([
            &format!("{toplevel_path}/bin/switch-to-configuration"),
            command
        ])
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|_| NixError::ConfigSwitch)
        .map(|_| ())
}
pub mod flakeref;
use std::process;
use std::str;
use serde::Deserialize;
use std::collections::HashMap;

pub use flakeref::FlakeReference;

#[derive(Debug)]
pub enum NixError {
    EvalError,
    BuildError,
    MultipleOutputPaths,
    NoOutputPath,
    ConfigSwitchError,
    ProfileSetError,
    DeserializationError,
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
        .map_err(|_| NixError::EvalError)?;

    let stdout_str = str::from_utf8(&build_output.stdout).expect("Failed to convert to string");
    let deserialized_vec: Vec<String> = serde_json::from_str(stdout_str)
        .map_err(|_| NixError::DeserializationError)?;
    Ok(deserialized_vec)
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
        .map_err(|_| NixError::EvalError)?;

    let stdout_str = str::from_utf8(&build_output.stdout).expect("Failed to convert to string");
    Ok(stdout_str.to_string())
}

#[derive(Debug, Deserialize)]
struct BuildOutput {
    drvPath: String,
    outputs: HashMap<String, String>,
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
        .map_err(|_| NixError::BuildError)?;

    let stdout_str = str::from_utf8(&build_output.stdout).expect("Failed to convert to string");
    let build_outputs: Vec<BuildOutput> = serde_json::from_str(stdout_str)
        .map_err(|_| NixError::DeserializationError)?;

    if build_outputs.len() != 1 {
        return Err(NixError::MultipleOutputPaths);
    }

    match build_outputs[0].outputs.get("out") {
        Some(out) => Ok(out.clone()),
        None => Err(NixError::NoOutputPath)
    }
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
        .map_err(|_| NixError::ProfileSetError)
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
        .map_err(|_| NixError::ConfigSwitchError)
        .map(|_| ())
}
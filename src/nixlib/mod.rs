pub mod flakeref;
use eyre::{Result, bail};
use std::process;
use std::str;
use serde::Deserialize;
use std::collections::HashMap;

pub use flakeref::FlakeReference;

pub fn nixos_configuration_attributes(flake_url: &str) -> Result<Vec<String>> {
    let build_output = process::Command::new("nix")
        .args([
            "eval",
            "--json",
            &format!("{flake_url}#nixosConfigurations"),
            "--apply",
            "builtins.attrNames"
        ])
        .stderr(process::Stdio::inherit())
        .output()?;

    if !build_output.status.success() {
        bail!("Flake {} doesn't exist or doesn't evaluate without errors", flake_url)
    }

    let stdout_str = str::from_utf8(&build_output.stdout).expect("Failed to convert to string");
    let deserialized_vec: Vec<String> = serde_json::from_str(stdout_str)?;
    Ok(deserialized_vec)
}

pub fn nixos_configuration_flakerefs(flake_url: &str) -> Result<Vec<FlakeReference>> {
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

pub fn nixos_fqdn(flake_reference: &FlakeReference) -> Result<String> {
    let build_output = process::Command::new("nix")
        .args([
            "eval",
            "--raw",
            &format!("{0}#nixosConfigurations.\"{1}\".config.networking.fqdn", flake_reference.url, flake_reference.attribute),
        ])
        .stderr(process::Stdio::inherit())
        .output()?;

    if !build_output.status.success() {
        bail!("NixOS configuration {} doesn't exist or doesn't evaluate without errors", flake_reference.attribute)
    }

    let stdout_str = str::from_utf8(&build_output.stdout).expect("Failed to convert to string");
    Ok(stdout_str.to_string())
}

#[derive(Debug, Deserialize)]
struct BuildOutput {
    drvPath: String,
    outputs: HashMap<String, String>,
}

pub fn toplevel_output_path(flake_reference: &FlakeReference) -> Result<String> {
    let build_output = process::Command::new("nom")
        .args([
            "build",
            "--json",
            &format!("{0}#nixosConfigurations.\"{1}\".config.system.build.toplevel", flake_reference.url, flake_reference.attribute),
        ])
        .stderr(process::Stdio::inherit())
        .output()?;

    if !build_output.status.success() {
        bail!("NixOS configuration {} doesn't exist or doesn't evaluate/build without errors", flake_reference.attribute)
    }

    let stdout_str = str::from_utf8(&build_output.stdout).expect("Failed to convert to string");
    let build_outputs: Vec<BuildOutput> = serde_json::from_str(stdout_str)?;

    if build_outputs.len() != 1 {
        bail!("Expected only one output path");
    }

    match build_outputs[0].outputs.get("out") {
        Some(out) => Ok(out.clone()),
        None => bail!("No output path")
    }
}

pub fn activate_profile(toplevel_path: &str) -> Result<()> {
    let build_output = process::Command::new("sudo")
        .args([
            "nix-env",
            "-p",
            "/nix/var/nix/profiles/system",
            "--set",
            toplevel_path
        ])
        .stderr(process::Stdio::inherit())
        .output()?;

    if !build_output.status.success() {
        bail!("Unable to switch profile")
    }

    Ok(())
}

pub fn switch_to_configuration(toplevel_path: &str, command: &str) -> Result<()> {
    let build_output = process::Command::new("sudo")
        .args([
            &format!("{toplevel_path}/bin/switch-to-configuration"),
            command
        ])
        .stderr(process::Stdio::inherit())
        .output()?;

    if !build_output.status.success() {
        bail!("Unable to switch configuration")
    }

    Ok(())
}
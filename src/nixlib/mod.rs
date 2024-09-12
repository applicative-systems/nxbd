pub mod flakeref;
use eyre::{Result, bail};
use std::process;
use std::str;

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

pub fn nixos_fqdn(flake_reference: &FlakeReference) -> Result<String> {
    let build_output = process::Command::new("nix")
        .args([
            "eval",
            "--raw",
            &format!("{0}#nixosConfigurations.\"{1}\".config.networking.fqdn", flake_reference.flake_path, flake_reference.attribute),
        ])
        .stderr(process::Stdio::inherit())
        .output()?;

    if !build_output.status.success() {
        bail!("NixOS configuration {} doesn't exist or doesn't evaluate without errors", flake_reference.attribute)
    }

    let stdout_str = str::from_utf8(&build_output.stdout).expect("Failed to convert to string");
    Ok(stdout_str.to_string())
}

pub fn toplevel_output_path(flake_reference: &FlakeReference) -> Result<String> {
    let build_output = process::Command::new("nom")
        .args([
            "build",
            "--print-out-paths",
            &format!("{0}#nixosConfigurations.\"{1}\".config.system.build.toplevel", flake_reference.flake_path, flake_reference.attribute),
        ])
        .stderr(process::Stdio::inherit())
        .output()?;

    if !build_output.status.success() {
        bail!("NixOS configuration {} doesn't exist or doesn't evaluate/build without errors", flake_reference.attribute)
    }

    let stdout_str = str::from_utf8(&build_output.stdout).expect("Failed to convert to string");
    Ok(stdout_str.to_string())
}


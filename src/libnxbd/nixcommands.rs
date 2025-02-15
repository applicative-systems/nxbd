use serde_json;
use serde_json::Value;
use std::fmt;
use std::fs;
use std::process;
use std::str;
use which::which;

use super::FlakeReference;

#[derive(Debug)]
pub enum NixError {
    Eval(String),
    Build,
    ConfigSwitch,
    ProfileSet,
    Deserialization,
    NoHostName,
    Copy,
}

impl fmt::Display for NixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eval(msg) => write!(f, "Evaluation error: {msg}"),
            Self::Build => write!(f, "Build failed"),
            Self::ConfigSwitch => write!(f, "Failed to switch configuration"),
            Self::ProfileSet => write!(f, "Failed to set profile"),
            Self::Deserialization => write!(f, "Failed to parse output"),
            Self::NoHostName => write!(f, "No hostname configured"),
            Self::Copy => write!(f, "Failed to copy to host"),
        }
    }
}

pub fn nixos_configuration_attributes(flake_url: &str) -> Result<Vec<String>, NixError> {
    let build_output = process::Command::new("nix")
        .args([
            "eval",
            "--json",
            &format!("{flake_url}#nixosConfigurations"),
            "--apply",
            "builtins.attrNames",
        ])
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|_| NixError::Eval("Failed to execute nix eval".to_string()))?;

    let stdout_str = str::from_utf8(&build_output.stdout).expect("Failed to convert to string");
    let attributes: Vec<String> =
        serde_json::from_str(stdout_str).map_err(|_| NixError::Deserialization)?;

    Ok(attributes)
}

pub fn nixos_configuration_flakerefs(flake_url: &str) -> Result<Vec<FlakeReference>, NixError> {
    let discovered_attrs = nixos_configuration_attributes(flake_url)?;
    let flakerefs = discovered_attrs
        .into_iter()
        .map(|x| FlakeReference {
            url: flake_url.to_string(),
            attribute: x,
        })
        .collect();
    Ok(flakerefs)
}

pub fn realise_toplevel_output_path(flake_reference: &FlakeReference) -> Result<String, NixError> {
    let (cmd, mut args) = match which("nom") {
        Ok(_) => ("nom", vec!["build"]),
        Err(_) => ("nix", vec!["build", "--no-link"]),
    };

    let target = format!(
        "{0}#nixosConfigurations.\"{1}\".config.system.build.toplevel",
        flake_reference.url, flake_reference.attribute
    );

    args.extend(["--json", &target]);

    let build_output = process::Command::new(cmd)
        .args(args)
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|_| NixError::Build)?;

    if !build_output.status.success() {
        return Err(NixError::Build);
    }

    let stdout_str = String::from_utf8_lossy(&build_output.stdout);
    let parsed: Vec<serde_json::Value> =
        serde_json::from_str(&stdout_str).map_err(|_| NixError::Deserialization)?;
    let first_result = parsed.first().ok_or(NixError::Deserialization)?;
    let out_path = first_result
        .get("outputs")
        .and_then(|o| o.get("out"))
        .and_then(|o| o.as_str())
        .ok_or(NixError::Deserialization)?;
    let parsed = vec![out_path.to_string()];
    Ok(parsed.into_iter().next().expect("Empty build output"))
}

pub fn activate_profile(
    toplevel_path: &str,
    use_sudo: bool,
    remote_host: Option<&str>,
) -> Result<(), NixError> {
    let mut command_vec = Vec::new();

    if let Some(host) = remote_host {
        command_vec.push("ssh");
        command_vec.push(host);
    }
    if use_sudo {
        command_vec.push("sudo");
    }

    command_vec.extend(vec![
        "nix-env",
        "-p",
        "/nix/var/nix/profiles/system",
        "--set",
        toplevel_path,
    ]);

    let (cmd, args) = command_vec.split_first().ok_or(NixError::ProfileSet)?;

    process::Command::new(cmd)
        .args(args)
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|_| NixError::ProfileSet)
        .map(|_| ())
}

pub fn switch_to_configuration(
    toplevel_path: &str,
    command: &str,
    use_sudo: bool,
    remote_host: Option<&str>,
) -> Result<(), NixError> {
    let mut command_vec = Vec::new();

    if let Some(host) = remote_host {
        command_vec.push("ssh");
        command_vec.push(host);
    }
    if use_sudo {
        command_vec.push("sudo");
    }

    let switch_path = format!("{toplevel_path}/bin/switch-to-configuration");
    command_vec.extend(vec![&switch_path, command]);

    let (cmd, args) = command_vec.split_first().ok_or(NixError::ConfigSwitch)?;

    process::Command::new(cmd)
        .args(args)
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|_| NixError::ConfigSwitch)
        .map(|_| ())
}

pub fn copy_to_host(path: &str, host: &str) -> Result<(), NixError> {
    let target = format!("ssh://{}", host);
    process::Command::new("nix")
        .args(["copy", "--substitute-on-destination", "--to", &target, path])
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|_| NixError::Copy)
        .map(|_| ())
}

#[derive(Debug)]
pub struct RemoteBuilder {
    pub ssh_host: String,
    pub system: String,
}

fn get_nix_config_value(key: &str) -> Result<String, NixError> {
    let output = process::Command::new("nix")
        .args(["show-config", "--json"])
        .output()
        .map_err(|_| NixError::Eval("Failed to execute nix show-config".to_string()))?;

    let config: Value = serde_json::from_str(
        str::from_utf8(&output.stdout)
            .map_err(|_| NixError::Eval("Invalid UTF-8 in nix show-config output".to_string()))?,
    )
    .map_err(|_| NixError::Eval("Failed to parse JSON output".to_string()))?;

    config
        .get(key)
        .and_then(|s| s.get("value"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| NixError::Eval(format!("{key} not found in nix config")))
}

pub fn get_system() -> Result<String, NixError> {
    get_nix_config_value("system")
}

pub fn get_remote_builders() -> Result<Vec<RemoteBuilder>, NixError> {
    let builders_value = get_nix_config_value("builders")?;

    let builders_str = if builders_value.starts_with('@') {
        fs::read_to_string(&builders_value[1..]).map_err(|_| {
            NixError::Eval(format!(
                "Failed to read builders file: {}",
                &builders_value[1..]
            ))
        })?
    } else {
        builders_value
    };

    let builders = builders_str
        .split(|c| c == ';' || c == '\n')
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts.as_slice() {
                [ssh_host, system] => Some(RemoteBuilder {
                    ssh_host: ssh_host.to_string(),
                    system: system.to_string(),
                }),
                _ => None,
            }
        })
        .collect();

    Ok(builders)
}

pub fn toplevel_derivation_paths(
    flake_reference: &FlakeReference,
) -> Result<(String, String), NixError> {
    let output = process::Command::new("nix")
        .args([
            "eval",
            "--json",
            &format!(
                "{0}#nixosConfigurations.\"{1}\".config.system.build.toplevel",
                flake_reference.url, flake_reference.attribute
            ),
            "--apply",
            "out: { inherit out; drv = out.drvPath; }",
        ])
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|_| NixError::Eval("Failed to execute nix eval".to_string()))?;

    if !output.status.success() {
        return Err(NixError::Eval("Failed to get paths".to_string()));
    }

    let paths: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| NixError::Eval("Failed to parse JSON output".to_string()))?;

    let out_path = paths
        .get("out")
        .and_then(|v| v.as_str())
        .ok_or_else(|| NixError::Eval("Missing out path in output".to_string()))?;

    let drv_path = paths
        .get("drv")
        .and_then(|v| v.as_str())
        .ok_or_else(|| NixError::Eval("Missing drv path in output".to_string()))?;

    Ok((out_path.to_string(), drv_path.to_string()))
}

pub fn realise_drv_remotely(drv_path: &str, host: &str) -> Result<String, NixError> {
    let output = process::Command::new("ssh")
        .args([host, "nix-store", "--realise", drv_path])
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|_| NixError::Build)?;

    if !output.status.success() {
        return Err(NixError::Build);
    }

    let path = String::from_utf8(output.stdout)
        .map_err(|_| NixError::Build)?
        .trim()
        .to_string();

    if path.is_empty() {
        return Err(NixError::Build);
    }

    Ok(path)
}

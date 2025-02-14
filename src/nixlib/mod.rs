pub mod configcheck;
pub mod deployinfo;
pub mod flakeref;
pub mod sshkeys;
pub mod userinfo;

use std::process;
use std::str;
use which::which;

pub use flakeref::FlakeReference;

#[derive(Debug)]
pub enum NixError {
    Eval(String),
    Build,
    ConfigSwitch,
    ProfileSet,
    Deserialization,
    NoHostName,
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

pub fn toplevel_output_path(flake_reference: &FlakeReference) -> Result<String, NixError> {
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

pub fn copy_to_host(toplevel_path: &str, host: &str) -> Result<(), NixError> {
    let target = format!("ssh://{}", host);
    process::Command::new("nix")
        .args(["copy", "--to", &target, toplevel_path])
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|_| NixError::ConfigSwitch)
        .map(|_| ())
}

use serde_json;
use serde_json::Value;
use std::fmt;
use std::fs;
use std::process;
use std::str;
use which::which;

use super::FlakeReference;

#[derive(Debug, Clone)]
pub enum NixError {
    Eval(String),
    Build,
    ConfigSwitch,
    ProfileSet,
    Deserialization,
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
            Self::Copy => write!(f, "Failed to copy to host"),
        }
    }
}

impl std::error::Error for NixError {}

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

// New helper module for command execution
mod command {
    use super::NixError;
    use std::process::{Command, Output};

    pub fn build_remote_command(remote_host: Option<&str>, use_sudo: bool) -> Vec<String> {
        let mut command_vec = Vec::new();
        if let Some(host) = remote_host {
            command_vec.extend(["ssh", host].map(String::from));
        }
        if use_sudo {
            command_vec.push("sudo".to_string());
        }
        command_vec
    }

    pub fn run_command(cmd: &str, args: &[&str], error: NixError) -> Result<Output, NixError> {
        Command::new(cmd)
            .args(args)
            .stderr(std::process::Stdio::inherit())
            .output()
            .map_err(|_| error)
    }

    pub fn run_remote_command(
        cmd: &[&str],
        remote_host: Option<&str>,
        use_sudo: bool,
        error: NixError,
    ) -> Result<Output, NixError> {
        let mut command = build_remote_command(remote_host, use_sudo);
        command.extend(cmd.iter().map(|s| s.to_string()));

        let (cmd, args) = command.split_first().ok_or_else(|| error.clone())?;

        run_command(
            cmd,
            args.iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .as_slice(),
            error,
        )
    }
}

// New helper module for JSON parsing
mod json {
    use super::NixError;
    use serde_json::Value;
    use std::str;

    pub fn parse_nix_json_output(output: &[u8]) -> Result<Value, NixError> {
        let stdout_str = str::from_utf8(output)
            .map_err(|_| NixError::Eval("Invalid UTF-8 in nix output".to_string()))?;

        serde_json::from_str(stdout_str)
            .map_err(|_| NixError::Eval("Failed to parse JSON output".to_string()))
    }
}

pub fn activate_profile(
    toplevel_path: &str,
    use_sudo: bool,
    remote_host: Option<&str>,
) -> Result<(), NixError> {
    command::run_remote_command(
        &[
            "nix-env",
            "-p",
            "/nix/var/nix/profiles/system",
            "--set",
            toplevel_path,
        ],
        remote_host,
        use_sudo,
        NixError::ProfileSet,
    )?;
    Ok(())
}

pub fn switch_to_configuration(
    toplevel_path: &str,
    command: &str,
    use_sudo: bool,
    remote_host: Option<&str>,
) -> Result<(), NixError> {
    let switch_path = format!("{toplevel_path}/bin/switch-to-configuration");
    command::run_remote_command(
        &[&switch_path, command],
        remote_host,
        use_sudo,
        NixError::ConfigSwitch,
    )?;
    Ok(())
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
#[allow(dead_code)]
pub struct RemoteBuilder {
    pub ssh_host: String,
    pub system: String,
}

pub fn get_nix_config_value(key: &str) -> Result<Value, NixError> {
    let output = command::run_command(
        "nix",
        &["config", "show", "--json"],
        NixError::Eval("Failed to execute nix config show".to_string()),
    )?;

    let config = json::parse_nix_json_output(&output.stdout)?;
    config
        .get(key)
        .and_then(|s| s.get("value"))
        .ok_or_else(|| NixError::Eval(format!("{key} not found in nix config")))
        .cloned()
}

pub fn get_system() -> Result<(String, Vec<String>), NixError> {
    let system = get_nix_config_value("system")?
        .as_str()
        .ok_or_else(|| NixError::Eval("system is not a string".to_string()))?
        .to_string();

    let extra_platforms = get_nix_config_value("extra-platforms")
        .map(|v| {
            v.as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default()
        })
        .unwrap_or_default();

    Ok((system, extra_platforms))
}

fn parse_builders(content: &str) -> Vec<RemoteBuilder> {
    // in the machines file the lines are separated by \n,
    // while they are separated by ; in the nix config when
    // they are inline
    content
        .split(|c| c == ';' || c == '\n')
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                Some(RemoteBuilder {
                    ssh_host: parts[0].to_string(),
                    system: parts[1].to_string(),
                })
            } else {
                None
            }
        })
        .collect()
}

pub fn get_remote_builders() -> Result<Vec<RemoteBuilder>, NixError> {
    let builders_value = get_nix_config_value("builders")?
        .as_str()
        .ok_or_else(|| NixError::Eval("builders value is not a string".to_string()))?
        .to_string();

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

    Ok(parse_builders(&builders_str))
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

pub fn realise_toplevel_output_paths(flake_references: &[FlakeReference]) -> Result<(), NixError> {
    let (cmd, mut args) = match which("nom") {
        Ok(_) => ("nom", vec!["build"]),
        Err(_) => ("nix", vec!["build", "--no-link"]),
    };

    // Build all targets in one command
    let targets: Vec<String> = flake_references
        .iter()
        .map(|fr| {
            format!(
                "{}#nixosConfigurations.\"{}\".config.system.build.toplevel",
                fr.url, fr.attribute
            )
        })
        .collect();

    args.extend(["--json"]);
    args.extend(targets.iter().map(String::as_str));

    command::run_command(cmd, &args, NixError::Build).map(|_| ())
}

pub fn reboot_host(host: &str) -> Result<(), NixError> {
    // Use systemctl to reboot, which will gracefully terminate the SSH connection
    let mut cmd = command::build_remote_command(Some(host), true);
    cmd.extend(["systemctl", "reboot"].iter().map(|&s| s.to_string()));

    let output = command::run_command(
        &cmd[0],
        &cmd[1..].iter().map(String::as_str).collect::<Vec<_>>(),
        NixError::Eval("Failed to initiate reboot".to_string()),
    )?;

    // If we get here, the command was sent successfully
    // The SSH connection will be terminated by the reboot
    // We consider this a success
    if !output.status.success() {
        return Err(NixError::Eval("Reboot command failed".to_string()));
    }

    Ok(())
}

#[derive(Debug)]
pub enum SystemStatus {
    Unreachable,
    Reachable {
        current_generation: String,
        needs_reboot: bool,
        uptime_seconds: u64,
        failed_units: usize,
    },
}

pub fn check_system_status(host: Option<&str>) -> Result<SystemStatus, NixError> {
    let status_script = r#"
        set -euo pipefail

        currentgen=$(readlink -f /nix/var/nix/profiles/system)
        uptime_sec=$(cat /proc/uptime | cut -d' ' -f1)
        failed_units=$(systemctl list-units --state=failed --no-legend | wc -l)

        # Check if reboot is needed
        booted="/run/booted-system"
        needs_reboot=0
        for component in initrd kernel kernel-modules; do
            if ! cmp -s "$booted/$component" "$currentgen/$component"; then
                needs_reboot=1
                break
            fi
        done

        echo "$currentgen"
        echo "$uptime_sec"
        echo "$failed_units"
        echo "$needs_reboot"
    "#;

    let output = run_script(status_script, host)?;

    if !output.status.success() {
        return Ok(SystemStatus::Unreachable);
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut lines = output_str.lines();

    // If any of these fail to parse, the system is considered unreachable
    let current_generation = match lines.next().map(|s| s.trim().to_string()) {
        Some(gen) if !gen.is_empty() => gen,
        _ => return Ok(SystemStatus::Unreachable),
    };

    let uptime_seconds = match lines.next().and_then(|s| {
        s.split_whitespace()
            .next()
            .and_then(|n| n.parse::<f64>().ok().map(|f| f as u64))
    }) {
        Some(uptime) => uptime,
        _ => return Ok(SystemStatus::Unreachable),
    };

    let failed_units = match lines.next().and_then(|s| s.parse::<usize>().ok()) {
        Some(units) => units,
        _ => return Ok(SystemStatus::Unreachable),
    };

    let needs_reboot = match lines.next().and_then(|s| s.parse::<u8>().ok()) {
        Some(1) => true,
        Some(0) => false,
        _ => return Ok(SystemStatus::Unreachable),
    };

    Ok(SystemStatus::Reachable {
        current_generation,
        needs_reboot,
        uptime_seconds,
        failed_units,
    })
}

pub fn run_script(script: &str, host: Option<&str>) -> Result<process::Output, NixError> {
    match host {
        Some(h) => {
            let mut cmd = std::process::Command::new("ssh");
            cmd.arg(h)
                .arg("bash")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::inherit());

            let mut child = cmd
                .spawn()
                .map_err(|_| NixError::Eval("Failed to spawn SSH".to_string()))?;

            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin
                    .write_all(script.as_bytes())
                    .map_err(|_| NixError::Eval("Failed to write to SSH stdin".to_string()))?;
            }

            child
                .wait_with_output()
                .map_err(|_| NixError::Eval("Failed to get SSH output".to_string()))
        }
        None => command::run_command("bash", &["-c", script], NixError::Build),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_builders_extended_format() {
        let input = "ssh-ng://builder@linux-builder aarch64-linux /etc/nix/builder_ed25519 4 1 kvm,benchmark,big-parallel - c3NoLWVkMjU1MTkgQUFBQUMzTnphQzFsWkRJMU5URTVBQUFBSUpCV2N4Yi9CbGFxdDFhdU90RStGOFFVV3JVb3RpQzVxQkorVXVFV2RWQ2Igcm9vdEBuaXhvcwo=";
        let builders = parse_builders(input);
        assert_eq!(builders.len(), 1);
        assert_eq!(builders[0].ssh_host, "ssh-ng://builder@linux-builder");
        assert_eq!(builders[0].system, "aarch64-linux");
    }

    #[test]
    fn test_parse_builders_semicolon_separated() {
        let input = "ssh://mac x86_64-darwin ; ssh://beastie x86_64-freebsd";
        let builders = parse_builders(input);
        assert_eq!(builders.len(), 2);
        assert_eq!(builders[0].ssh_host, "ssh://mac");
        assert_eq!(builders[0].system, "x86_64-darwin");
        assert_eq!(builders[1].ssh_host, "ssh://beastie");
        assert_eq!(builders[1].system, "x86_64-freebsd");
    }

    #[test]
    fn test_parse_builders_newline_separated() {
        let input = "ssh://mac x86_64-darwin\nssh://beastie x86_64-freebsd";
        let builders = parse_builders(input);
        assert_eq!(builders.len(), 2);
        assert_eq!(builders[0].ssh_host, "ssh://mac");
        assert_eq!(builders[0].system, "x86_64-darwin");
        assert_eq!(builders[1].ssh_host, "ssh://beastie");
        assert_eq!(builders[1].system, "x86_64-freebsd");
    }
}

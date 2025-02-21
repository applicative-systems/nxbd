use super::sshkeys::SshKeyInfo;
use super::{FlakeReference, NixError};

use serde::Deserialize;
use std::str;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::module_name_repetitions)]
pub struct ConfigInfo {
    pub amd_microcode: bool,
    pub boot_grub: bool,
    pub boot_grub_generations: Option<i32>,
    pub boot_is_container: bool,
    pub boot_systemd: bool,
    pub boot_systemd_generations: Option<i32>,
    pub command_not_found: bool,
    pub doc_dev_enable: bool,
    pub doc_doc_enable: bool,
    pub doc_enable: bool,
    pub doc_info_enable: bool,
    pub doc_man_enable: bool,
    pub doc_nixos_enabled: bool,
    pub font_fontconfig_enable: bool,
    pub fqdn: Option<String>,
    pub fqdn_or_host_name: String,
    pub host_name: String,
    pub intel_microcode: bool,
    pub is_x86: bool,
    pub journald_extra_config: String,
    pub log_refused_connections: bool,
    pub networking_firewall_enabled: bool,
    pub nginx_brotli: bool,
    pub nginx_enabled: bool,
    pub nginx_gzip: bool,
    pub nginx_optimisation: bool,
    pub nginx_proxy: bool,
    pub nginx_tls: bool,
    pub nix_auto_optimise_store: bool,
    pub nix_extra_options: String,
    pub nix_gc: bool,
    pub nix_optimise_automatic: bool,
    pub nix_trusts_wheel: bool,
    pub ssh_enabled: bool,
    pub ssh_password_authentication: bool,
    pub stub_ld: bool,
    pub sudo_enabled: bool,
    pub sudo_wheel_only: bool,
    pub system: String,
    pub toplevel_drv: String,
    pub toplevel_out: String,
    pub users: Vec<NixUser>,
    pub users_mutable: bool,
    pub wheel_needs_password: bool,
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
    let nix_expr = r#"{ config, pkgs, ... }:
        let
          tryOrNull = x:
            let r = builtins.tryEval x;
            in if r.success then r.value else null;
        in
        {
            inherit (pkgs) system;
            users = map (user: {
                inherit (user) name extraGroups;
                sshKeys = user.openssh.authorizedKeys.keys or [];
            }) (builtins.filter
                (user: (user.isNormalUser or false))
                (builtins.attrValues config.users.users));

            amdMicrocode = config.hardware.cpu.amd.updateMicrocode;
            bootGrub = config.boot.loader.grub.enable;
            bootGrubGenerations = config.boot.loader.grub.configurationLimit;
            bootIsContainer = config.boot.isContainer;
            bootSystemd = config.boot.loader.systemd-boot.enable;
            bootSystemdGenerations = config.boot.loader.systemd-boot.configurationLimit;
            commandNotFound = config.programs.command-not-found.enable;
            docDevEnable = config.documentation.dev.enable;
            docDocEnable = config.documentation.doc.enable;
            docEnable = config.documentation.enable;
            docInfoEnable = config.documentation.info.enable;
            docManEnable = config.documentation.man.enable;
            docNixosEnabled = config.documentation.nixos.enable;
            fontFontconfigEnable = config.fonts.fontconfig.enable;
            fqdn = tryOrNull config.networking.fqdn;
            fqdnOrHostName = config.networking.fqdnOrHostName;
            hostName = config.networking.hostName;
            intelMicrocode = config.hardware.cpu.intel.updateMicrocode;
            isX86 = pkgs.stdenv.hostPlatform.isx86;
            journaldExtraConfig = config.services.journald.extraConfig;
            logRefusedConnections = config.networking.firewall.logRefusedConnections;
            networkingFirewallEnabled = config.networking.firewall.enable;
            nginxBrotli = config.services.nginx.recommendedBrotliSettings;
            nginxEnabled = config.services.nginx.enable;
            nginxGzip = config.services.nginx.recommendedGzipSettings;
            nginxOptimisation = config.services.nginx.recommendedOptimisation;
            nginxProxy = config.services.nginx.recommendedProxySettings;
            nginxTls = config.services.nginx.recommendedTlsSettings;
            nixAutoOptimiseStore = config.nix.settings.auto-optimise-store;
            nixExtraOptions = config.nix.extraOptions;
            nixGc = config.nix.gc.automatic;
            nixOptimiseAutomatic = config.nix.optimise.automatic;
            nixTrustsWheel = builtins.elem "@wheel" config.nix.settings.trusted-users;
            sshEnabled = config.services.openssh.enable;
            sshPasswordAuthentication = config.services.openssh.settings.PasswordAuthentication;
            stubLd = config.environment.stub-ld.enable;
            sudoEnabled = config.security.sudo.enable;
            sudoWheelOnly = config.security.sudo.execWheelOnly;
            toplevelDrv = config.system.build.toplevel.drvPath;
            toplevelOut = config.system.build.toplevel;
            usersMutable = config.users.mutableUsers;
            wheelNeedsPassword = config.security.sudo.wheelNeedsPassword;
        }"#;

    let output = std::process::Command::new("nix")
        .args([
            "eval",
            "--json",
            &format!(
                "{}#nixosConfigurations.\"{}\"",
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

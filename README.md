<div align="center">

# nxbd

The smart NixOS Configuration Check, Build, and Deploy Tool

**Presented to you and maintained by <a href="https://applicative.systems/">Applicative Systems</a> and <a href="https://nixcademy.com/">Nixcademy</a>**

<p>
<a href="https://github.com/applicative-systems/nxbd/actions/workflows/check.yml"><img src="https://github.com/applicative-systems/nxbd/actions/workflows/check.yml/badge.svg"/></a>
<a href="https://matrix.to/#/#applicative.systems:matrix.org"><img src="https://img.shields.io/badge/Support-%23applicative.systems-blue"/></a>
</p>

</div>

`nxbd` is a lightweight, safety-focused NixOS deployment tool that helps you manage multiple NixOS systems with confidence. Unlike other deployment tools, `nxbd` focuses on validating your configurations before deployment to prevent common pitfalls and system lockouts.

## Why nxbd?

`nxbd` tries to be 100% compatible with standard NixOS configurations, just like `nixos-rebuild`.
It does not require you to put any special code in your flake.nix, or add new configuration files.

While tools like `nixos-rebuild` and `deploy-rs` and others handle the deployment process, they don't validate whether your configuration is actually safe to deploy. This can lead to situations where you:

- Lock yourself out of SSH access
- Break sudo permissions
- Deploy to the wrong system
- Misconfigure critical services
- Create configurations that waste disk space

`nxbd` runs extensive pre-deployment checks to catch these issues before they become problems. It verifies:

- SSH access and key configuration
- Sudo and wheel group permissions
- Boot loader generation limits
- Journald space management
- Nix feature enablement
- Documentation settings (to reduce closure size)
- CPU microcode updates
- Nginx recommended settings
- ...and more

## Examples

### Check a System Configuration

```bash
# Check the local system
nxbd check .#hostname

# Check all systems in the flake in the current directory
nxbd check

# Check multiple remote systems
nxbd check .#{server1,server2,server3}

# Show detailed information during checks
nxbd check -v .#server1
```

Example output:

```console
$ nxbd check --verbose .#dash

System Configurations:

=== .#dash ===
remote_deployment - Remote Deployment Support: ✓
Checks if the system has the required configuration to safely perform remote deployments

  ssh_enabled: ✓
  sudo_enabled: ✓
  wheel_passwordless: ✓
  nix_trusts_wheel: ✓
  user_access: ✓
  user_in_wheel: ✓
sudo_security - Sudo Security Settings: ✗
Checks if sudo is configured securely

  wheel_only: ✗
    - Only wheel group members should be allowed to use sudo
      Set security.sudo.execWheelOnly = true

firewall_settings - Firewall settings: ✓
Check whether firewall is configured correctly

  log_refused_connections: ✓
boot_configuration_limit - Boot Configuration Limit: ✓
Checks if system configuration generations are reasonably limited to prevent disk space waste

  boot_systemd_generations: ✓
  boot_grub_generations: ✓
disk_space_management - Disk Space Management: ✓
Checks whether the optimisations and limits for disk space are configured

  journald_limits: ✓
  nix_optimise_automatic: ✓
nix_flakes - Nix Flakes: ✗
Checks if flakes are enabled

  nix_extra_options: ✗
    - Nix features should include nix-command and flakes
      Add 'experimental-features = nix-command flakes' to nix.extraOptions

disable_documentation - Disable Documentation on Servers: ✗
Checks if documentation is disabled on servers to reduce closure size

  doc_nixos_enabled: ✗
    - NixOS documentation should be disabled
      Set documentation.nixos.enable = false

  doc_enable: ✗
    - General documentation should be disabled
      Set documentation.enable = false

  doc_dev_enable: ✓
  doc_doc_enable: ✗
    - Doc documentation should be disabled
      Set documentation.doc.enable = false

  doc_info_enable: ✗
    - Info documentation should be disabled
      Set documentation.info.enable = false

  doc_man_enable: ✗
    - Man pages should be disabled
      Set documentation.man.enable = false

enable_cpu_microcode_updates - Enable CPU Microcode Updates on x86: ✓
Checks if CPU microcode updates are enabled on x86 systems

  cpu_microcode: ✓
nginx_recommended_settings - Nginx Recommended Settings: ✗
Checks if nginx has recommended settings enabled

  nginx_brotli: ✗
    - Brotli compression should be enabled
      Set services.nginx.recommendedBrotliSettings = true

  nginx_gzip: ✓
  nginx_optimisation: ✓
  nginx_proxy: ✓
  nginx_tls: ✗
    - TLS settings should be enabled
      Set services.nginx.recommendedTlsSettings = true

garbage_collection - Garbage Collection: ✓
Checks whether the Nix garbage collection is configured correctly

  nix_gc: ✓
```

### Deploy to Remote Systems

The `switch-remote` command builds and deploys to remote systems.
It derives the deployment target address from the `networking.fqdnOrHostName` attribute in the system configuration.

```bash
# Build and deploy to a single system
nxbd switch-remote .#server1

# Deploy to multiple systems
nxbd switch-remote .#{server1,server2,server3}
```

### Local System Management

```bash
# Switch the local system
nxbd switch-local .#hostname

# Build without deploying
nxbd build .#hostname
```

### List Available Checks

```bash
nxbd checks
```

## Installation

### Just run it from the internet

```bash
nix run github:applicative-systems/nxbd
```

### Add to your flake.nix

Add to your flake.nix:

```nix
{
  inputs.nxbd.url = "github:applicative-systems/nxbd";

  outputs = { self, nixpkgs, nxbd }: {
    # For your packages
    packages.x86_64-linux.nxbd = nxbd.packages.x86_64-linux.default;

    # Or in your NixOS configuration
    nixosConfigurations.hostname = nixpkgs.lib.nixosSystem {
      modules = [
        # ...
        { environment.systemPackages = [ nxbd.packages.x86_64-linux.default ]; }
      ];
    };
  };
}
```

### Direct Installation in the Nix Profile

```bash
nix profile install github:applicative-systems/nxbd
```

## Requirements

- Nix with flakes enabled
- SSH agent running with keys (for remote deployment)
- Sudo access on target systems

## Safety Features

- Pre-deployment configuration validation
- SSH key verification
- Sudo permission checks
- Automatic hostname verification
- Detailed error reporting
- Safe rollback support (via NixOS generations)

## Contributing

Contributions are welcome! Feel free to open issues or submit pull requests.

## Commercial Support

Do you need some additions to this tool to support your internal processes?
Would you like to have similar but new open or closed source tool?

Contact us via mail
[hello@applicative.systems](mailto:hello@applicative.systems)
or schedule a meeting: <https://nixcademy.com/meet>

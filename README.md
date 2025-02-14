# nxbd - NixOS Build & Deploy

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
  inputs.nxbd.url = "github:yourusername/nxbd";
  
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
nix profile install github:yourusername/nxbd
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

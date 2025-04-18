<div align="center">

# nxbd

The smart NixOS Configuration Check, Build, and Deploy Tool

**Developed and maintained by [Applicative Systems](https://applicative.systems/) and [Nixcademy](https://nixcademy.com/)**

<p>
<a href="https://github.com/applicative-systems/nxbd/actions/workflows/check.yml"><img src="https://github.com/applicative-systems/nxbd/actions/workflows/check.yml/badge.svg"/></a>
<a href="https://matrix.to/#/#applicative.systems:matrix.org"><img src="https://img.shields.io/badge/Support-%23applicative.systems-blue"/></a>
</p>

</div>

`nxbd` is a lightweight, safety-focused NixOS deployment tool that helps you manage multiple NixOS systems with confidence. It validates your configurations before deployment to prevent common pitfalls and system lockouts.

## Why nxbd?

- **Zero Configuration**: Works with vanilla NixOS configurations - no special code or extra files needed
- **Safety First**: Prevents common deployment issues before they happen
- **Smart Deployment**: Handles complex deployment scenarios automatically
- **System Health**: Built-in monitoring and status checks

### What Problems Does It Solve?

While tools like `nixos-rebuild` handle deployment, they don't validate whether your configuration is safe to deploy. `nxbd` prevents common issues like:

- SSH access lockouts
- Broken sudo permissions
- Wrong system deployments
- Misconfigured critical services
- Disk space waste
- Service configuration errors

## Quick Start

```bash
# Check configuration safety
nxbd check

# Deploy to remote systems
nxbd switch-remote .#server1 .#server2

# Monitor system status
nxbd status
```

## Installation

### Quick Run

```bash
nix run github:applicative-systems/nxbd
```

### Add to flake.nix

```nix
{
  inputs.nxbd.url = "github:applicative-systems/nxbd";

  outputs = { self, nixpkgs, nxbd }: {
    # For your packages
    packages.x86_64-linux.nxbd = nxbd.packages.x86_64-linux.default;

    # Or in your NixOS configuration
    nixosConfigurations.hostname = nixpkgs.lib.nixosSystem {
      modules = [
        { environment.systemPackages = [ nxbd.packages.x86_64-linux.default ]; }
      ];
    };
  };
}
```

### Direct Installation

```bash
nix profile install github:applicative-systems/nxbd
```

## Requirements

- Nix with flakes enabled
- SSH agent with keys (for remote deployment)
- Sudo access on target systems

## Documentation

Visit our [documentation site](https://applicative.systems/nxbd/) for:

- Detailed command reference
- Configuration check explanations
- Best practices and guides
- Example configurations

## Professional Services

We offer commercial support to help you succeed with `nxbd`:

- **Custom Development**: Tailored features for your needs
- **Integration Support**: Help with your deployment workflows
- **Training**: Expert guidance for your team
- **Consulting**: Infrastructure optimization

Contact us:

- 📧 [hello@applicative.systems](mailto:hello@applicative.systems)
- 🤝 [Schedule a meeting](https://nixcademy.com/meet)

## Community

- Join our [Matrix channel](https://matrix.to/#/#applicative.systems:matrix.org)
- Report issues on [GitHub](https://github.com/applicative-systems/nxbd/issues)
- Contribute via [Pull Requests](https://github.com/applicative-systems/nxbd/pulls)

## License

[MIT License](LICENSE)

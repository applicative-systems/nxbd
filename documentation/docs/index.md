# The `nxbd` Tool

The smart NixOS Configuration Check, Build, and Deploy Tool

**Developed and maintained by [Applicative Systems](https://applicative.systems/) and [Nixcademy](https://nixcademy.com/)**

## What is nxbd?

`nxbd` is a lightweight, safety-focused NixOS deployment tool that helps you manage multiple NixOS systems with confidence. Unlike other deployment tools, `nxbd` focuses on validating your configurations before deployment to prevent common pitfalls and system lockouts.

## Use Cases

`nxbd` is perfect for teams and individuals who:

- Need to deploy their local or (multiple) remote systems
    - Deploy from macOS to NixOS machines (via [Linux Builder](https://nixcademy.com/posts/macos-linux-builder/))
    - Deploy without complex `nixos-rebuild` arguments
- Want to prevent system lockouts and configuration errors before they happen
- Need to monitor server configurations and system health
- Want a simple, standards-compliant tool without extra configuration overhead

## Key Features

- **Zero Configuration**: Works with vanilla system flakes - no additional configuration files needed
- **Smart Deployment**: Automatically handles all the complexities that `nixos-rebuild` requires manual configuration for
- **Safety First**: Comprehensive pre-deployment checks for:
    - SSH access and key configuration
    - Sudo and wheel group permissions
    - Boot loader and disk space management
    - Service configurations and security settings
- **Intelligent Building**: Automatically builds on target hosts when local building isn't possible
- **System Health Monitoring**: Easy status checks for updates, reboots, and service health

## Quick Start

```console
# Run all the checks
$ nxbd check

# Fix or ignore failed checks
$ nxbd check --save-ignore

# Deploy with automatic reboots if needed
$ nxbd switch-remote --reboot

# Monitor system status
$ nxbd status
```

## Documentation

- [Command Reference](commands/index.md)
- [Configuration Checks](checks/index.md)
- [Installation Guide](commands/installation.md)

## Professional Support

Need help integrating `nxbd` into your infrastructure?

- **Custom Development**: Tailored features for your specific needs
- **Integration Support**: Help with your deployment workflows
- **Training**: Expert guidance for your team

Contact us:

- Email: [hello@applicative.systems](mailto:hello@applicative.systems)
- Schedule a meeting: [nixcademy.com/meet](https://nixcademy.com/meet)

## Community

Join our community:
- Matrix: [#applicative.systems:matrix.org](https://matrix.to/#/#applicative.systems:matrix.org)
- GitHub: [applicative-systems/nxbd](https://github.com/applicative-systems/nxbd)

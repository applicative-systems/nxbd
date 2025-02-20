# The `nxbd` Tool

The smart NixOS Configuration Check, Build, and Deploy Tool

**Developed and maintained by [Applicative Systems](https://applicative.systems/) and [Nixcademy](https://nixcademy.com/)**

## What is `nxbd`?

`nxbd` is a lightweight, safety-focused NixOS deployment tool that helps you manage multiple NixOS systems with confidence. Unlike other deployment tools, `nxbd` focuses on validating your configurations before deployment to prevent common pitfalls and system lockouts.

## Features

<div class="grid cards" markdown>

-   :material-server-network:{ .lg .middle } __Deploy Anywhere__

    ---

    Deploy to local or remote NixOS systems, even from macOS, with automatic remote building support.

    [:octicons-arrow-right-24: command reference](commands/index.md)

-   :material-shield-lock:{ .lg .middle } __Safety First__

    ---

    Prevent SSH lockouts, broken sudo permissions, and configuration errors before they
    happen with comprehensive pre-deployment checks for SSH access, sudo/wheel permissions,
    boot loader/disk space, service configs, and security settings.

    [:octicons-arrow-right-24: Security Checks](checks/index.md)

-   :material-monitor-dashboard:{ .lg .middle } __System Health__

    ---

    Monitor updates, service status, and reboot requirements across all your systems.

    [:octicons-arrow-right-24: Monitoring](commands/status.md)

-   :material-cog:{ .lg .middle } __Zero Configuration__

    ---

    Works with vanilla NixOS configurations - no special code or extra files needed.
    Automatically handles all the complexities that `nixos-rebuild` requires manual
    configuration for.

-   :material-eye:{ .lg .middle } __Fancy Optics__

    ---

    Enjoy a polished user experience with real-time build progress and output
    thanks to [`nix-output-monitor`](https://github.com/maralorn/nix-output-monitor)
    integration (when available).

</div>

## Quick Start

```console
# Run all the checks
$ nxbd check

# Fix or ignore failed checks
$ nxbd check --save-ignore

# Deploy with automatic reboots if needed
$ nxbd switch-remote --reboot .#machine1 .#machine2

# Monitor system status
$ nxbd status
```

## Example output

=== "nxbd check"

    ```console
    $ nxbd check .#dash .#marketing
    Reading configurations of .#dash .#marketing...

    === .#dash ===

    sudo_security - Sudo Security Settings (1 checks, 0 passed, 0 ignored)
    Checks if sudo is configured securely

      ❌ wheel_only - Only wheel group members should be allowed to use sudo
        - Set security.sudo.execWheelOnly = true

    === .#marketing ===
    ✅ 26 checks passed (8 ignored fails)

    Error: The following checks failed:

    System .#dash:
      - sudo_security.wheel_only

    To proceed, either:
     - Fix the failing checks
     - Run 'nxbd check --save-ignore' to ignore these checks
    ```

=== "nxbd status"

    ```console
    $ nxbd status .#dash .#marketing
    Reading configurations of .#dash .#marketing...
    Querying status of dash.applicative.systems marketing.applicative.systems...

    System Status:

    === .#dash ===
      ✗ systemd units: 1 failed
      ✓ System generation up to date
      ✓ Reboot required: no
        Uptime: 0d 16h 12m

    === .#marketing ===
      ✓ systemd units: all OK
      ✓ System generation up to date
      ✓ Reboot required: no
        Uptime: 0d 16h 12m
    ```

## Documentation

- [Command Reference](commands/index.md)
- [Configuration Checks](checks/index.md)

## Professional Support

Need help integrating `nxbd` into your infrastructure?

- **Custom Development**: Tailored features for your specific needs
- **Integration Support**: Help with your deployment workflows
- **Training**: Expert guidance for your team

Contact us:

- Email: [hello@applicative.systems](mailto:hello@applicative.systems)
- Schedule a meeting: [nixcademy.com/meet](https://nixcademy.com/meet)

<div class="grid cards" markdown>

-   [![Nixcademy](assets/nixcademy.svg){ width="400" }](https://nixcademy.com)

    Battle-tested corporate Nix & NixOS trainings

-   [![Applicative Systems GmbH](assets/applicative-systems.svg){ width="200" }](https://applicative.systems)

    Software Engineering Consulting

</div>

## Community

Join our community:
- Matrix: [#applicative.systems:matrix.org](https://matrix.to/#/#applicative.systems:matrix.org)
- GitHub: [applicative-systems/nxbd](https://github.com/applicative-systems/nxbd)

# The `nxbd` Tool

The integrated NixOS Build, Config Check, and Deploy Tool.

## Use Cases

`nxbd` is built for users who:

- Need to deploy their local or (multiple) remote systems
    - also from macOS to NixOS machines
      (requires [Linux Builder](https://nixcademy.com/posts/linux-builder))
    - without having to assemble all the `nixos-rebuild` arguments manually
- Want to check configurations for basic sanity before deploying potentially
  broken or insecure configurations.
- Want a simple command to check if a server configuration is already up to date
    - or if it is up to date, but still needs a reboot
    - and if all systemd services are OK
- Don't want to learn how to use one of the many famous deployment tools that
  require them to add more configuration to the repository than just the
  `flake.nix` with the NixOS configurations.

## Features and Design

- `nxbd` works on *vanilla system flakes* that enumerate one or multiple NixOS
  configurations in the `nixosConfigurations` flake output category.
    - This is in contrast to other famous deployment tools which require
      additional configuration files next to your NixOS configurations.
- It uses the same commands for evaluating, building, and deploying that
  `nixos-rebuild` does, but discovers all necessary information from the NixOS
  configurations themselves.
- In contrast to `nixos-rebuild`, you don't need to provide:
    - `--target-host <hostname>`
    - `--fast` if you are deploying to NixOS machines from maOS
    - `--use-remote-sudo`
    - `--use-substitutes`
- `nxbd` automatically attempts to build configurations on the target host that
  it cannot build locally or on configured distributed builders.
- `nxbd` assumes that you want to deploy with your local user account via SSH
  and checks if the NixOS system is configured in a way that you are not locked
  out after the deployment.
- `nxbd` runs a [whole list of known best practise checks](checks/index.md) on
  the NixOS configurations and aborts the deployment if it encounters any failed
  checks.
- All checks can be ignored.

## Example Usage

Check, build, and deploy all remote systems that are defined in the `flake.nix`
file in the current working directory:

```console
# Run all the checks
$ nxbd check

# Fix all failed checks.
# If you want to ignore some of the checks, run:
$ nxbd check --save-ignore

# Build and deploy (allow reboots if necessary with --reboot)
$ nxbd switch-remote --reboot

# Check system status
$ nxbd status
```

`nxbd` assumes that the target hosts are reachable via SSH via the host address
that is defined by the configuration field `networking.fqdnOrHostName`
(which is defined by `networking.hostName` and the optional `networking.fqdn`
fields).

Please have a look at the [Command reference](commands/index.md) for more
information about the individual commands and their flags.

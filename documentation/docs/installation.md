# Installation

## Just launch `nxbd`

To launch the latest and greatest version of `nxbd` without installing it,
use `nix run`:

```console
> nix run github:applicative-systems/nxbd
Build and deploy NixOS systems using flakes

Usage: nxbd [OPTIONS] <COMMAND>

Commands:
  build          Build NixOS configurations without deploying
  switch-remote  Deploy configurations to remote systems
  switch-local   Deploy configuration to the local system
  check          Run configuration checks
  checks         List all available configuration checks
  status         Show status of NixOS systems
  help           Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose  Show detailed information during execution
  -h, --help     Print help (see more with '--help')
```

## Nix Shell

To launch a new temporary shell that provides `nxbd`, run:

```console
$ nix shell github:applicative-systems/nxbd

$ nxbd --help
A tool for building and deploying NixOS systems using flakes. It supports local and remote deployment, configuration checks, and automated system updates.

Usage: nxbd [OPTIONS] <COMMAND>

Commands:
  build          Build NixOS configurations without deploying
  switch-remote  Deploy configurations to remote systems
  switch-local   Deploy configuration to the local system
  check          Run configuration checks
  checks         List all available configuration checks
  status         Show status of NixOS systems
  help           Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose
          Show detailed information during execution

  -h, --help
          Print help (see a summary with '-h')
```

## Permanent Nix profile installation

To install `nxbd` in your user profile, run:

```console
$ nix profile install github:applicative-systems/nxbd

$ nix profile list
Name:               nxbd
Flake attribute:    packages.aarch64-darwin.default
Original flake URL: github:applicative-systems/nxbd
Locked flake URL:   github:applicative-systems/nxbd/90aa910a3c2e3a10f8ec5109b36e332c5177bbc8?narHash=sha256-GKu17rcHu36lqb57XK8L8hNBS/CLNd/jbZfFMg4s8j8%3D
Store paths:        /nix/store/fn54xdiwg6fmb82f9ax8dcx9qd7njhnk-nxbd
```

You can upgrade all the apps in your profile with `nix profile upgrade --all`.

## Flake developer shell

To integrate `nxbd` in the flakes of e.g. your infrastructure repos, add it like
this to your `flake.nix` file:

### Vanilla Flakes

```nix
{
  description = "My infrastructure flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

    # 1.) add nxbd input
    nxbd.url = "github:applicative-systems/nxbd";
  };

  outputs =
    {
      self,
      nixpkgs,
      nxbd, # 2.) add nxbd reference
    }:
    let
      system = "aarch64-darwin";
    in
    {
      devShells.${system}.default = nixpkgs.legacyPackages.${system}.mkShell {
        nativeBuildInputs = [
          # 3.) Add nxbd package to your existing dev shell definition
          nxbd.packages.${system}.default
        ];
      };
    };
}
```

### `flake-utils` Flakes

```nix
{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";

    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

    nxbd.url = "github:applicative-systems/nxbd";
  };

  outputs =
    {
      self,
      flake-utils,
      nixpkgs,
      nxbd,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (system: {
      devShells.default = nixpkgs.legacyPackages.${system}.mkShell {
        nativeBuildInputs = [
          nxbd.packages.${system}.default
        ];
      };
    });
}
```

### `flake-parts` Flakes

```nix
{
  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";

    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    nxbd.url = "github:applicative-systems/nxbd";
  };

  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
      ];
      perSystem =
        { pkgs, system, ... }:
        {
          devShells.default = pkgs.mkShell {
            nativeBuildInputs = [
              inputs.nxbd.packages.${system}.default
            ];
          };
        };
    };
}
```

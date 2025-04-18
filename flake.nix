{
  description = "nxbd - NixOS Build and Deploy Tool";

  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    flake-parts.inputs.nixpkgs-lib.follows = "nixpkgs";

    mkdocs-flake.url = "github:applicative-systems/mkdocs-flake";

    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    treefmt-nix.url = "github:numtide/treefmt-nix";
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
      imports = [
        inputs.mkdocs-flake.flakeModules.default
      ];
      perSystem =
        {
          config,
          lib,
          pkgs,
          system,
          ...
        }:
        let
          treefmtEval = inputs.treefmt-nix.lib.evalModule pkgs {
            projectRootFile = "flake.nix";
            programs = {
              deadnix.enable = true;
              # disabled for now because mkdocs wants 4 spaces indent for
              # multi level bullet point lists, but mdformat doesn't allow it.
              mdformat.enable = false;
              nixfmt.enable = true;
              rustfmt.enable = true;
              shfmt.enable = true;
              statix.enable = true;
              taplo.enable = true;
            };
          };
        in
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [
              inputs.self.overlays.default
            ];
            config = { };
          };

          packages = {
            inherit (pkgs) nxbd;
            default = config.packages.nxbd;
          };

          devShells.default = pkgs.mkShell {
            inputsFrom = [ config.packages.nxbd ];
            nativeBuildInputs = [
              pkgs.cargo-edit
              pkgs.cargo-watch
              pkgs.clippy
              treefmtEval.config.build.wrapper
            ];
          };

          documentation.mkdocs-root = pkgs.runCommand "documentation-root" { } ''
            mkdir $out
            cp -r ${./documentation}/* "$out"
            chmod -R 777 "$out"/*
            ${pkgs.nxbd}/bin/nxbd generate-docs "$out/docs"
          '';

          formatter = treefmtEval.config.build.wrapper;

          checks =
            config.packages
            // lib.optionalAttrs (pkgs.stdenv.isx86_64 && pkgs.stdenv.isLinux) {
              # Should run on all CPUs, but first we need to make the system
              # attribute inside the config a bit more dynamic.
              switch-local = pkgs.testers.runNixOSTest ./tests/switch-local.nix;
              switch-remote = pkgs.testers.runNixOSTest ./tests/switch-remote.nix;
            }
            // {
              formatting = treefmtEval.config.build.check inputs.self;
            };

          apps.watch-documentation = lib.mkForce {
            type = "app";
            program = "";
          };
        };
      flake = {
        overlays.default = import ./overlay.nix;
      };
    };

  nixConfig = {
    extra-substituters = [
      "https://appsys.cachix.org"
    ];
    extra-trusted-public-keys = [
      "appsys.cachix.org-1:VoZof6Mp3Aqlj3tQ21wFdxW0lhHTzAu/5q04LYUtXM8="
    ];
  };
}

{
  description = "nxbd - NixOS Build and Deploy Tool";

  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    flake-parts.inputs.nixpkgs-lib.follows = "nixpkgs";

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
              mdformat.enable = true;
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

          formatter = treefmtEval.config.build.wrapper;

          checks =
            config.packages
            // lib.optionalAttrs (pkgs.stdenv.isx86_64 && pkgs.stdenv.isLinux) {
              # Should run on all CPUs, but first we need to make the system
              # attribute inside the config a bit more dynamic.
              switch-local = pkgs.testers.runNixOSTest ./tests/switch-test.nix;
            }
            // {
              formatting = treefmtEval.config.build.check inputs.self;
            };
        };
      flake = {
        overlays.default = import ./overlay.nix;
      };
    };
}

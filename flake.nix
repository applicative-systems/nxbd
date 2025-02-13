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
          pkgs,
          system,
          ...
        }:
        let
          treefmtEval = inputs.treefmt-nix.lib.evalModule pkgs {
            projectRootFile = "flake.nix";
            programs = {
              deadnix.enable = true;
              nixfmt.enable = true;
              rustfmt.enable = true;
              statix.enable = true;
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
              pkgs.clippy
              pkgs.cargo-edit
            ];
          };

          formatter = treefmtEval.config.build.wrapper;

          checks = config.packages // {
            formatting = treefmtEval.config.build.check inputs.self;
          };
        };
      flake = {
        overlays.default = import ./overlay.nix;
      };
    };
}

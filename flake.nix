{
  description = "nxbd - NixOS Build and Deploy Tool";

  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    flake-parts.inputs.nixpkgs-lib.follows = "nixpkgs";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = inputs: inputs.flake-parts.lib.mkFlake { inherit inputs; } {
    systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];
    perSystem = { config, self', inputs', pkgs, system, ... }: {
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
    };
    flake = {
      overlays.default = import ./overlay.nix;
    };
  };
}

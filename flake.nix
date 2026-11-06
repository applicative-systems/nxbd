{
  description = "nxbd - NixOS Build and Deploy Tool";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    mkdocs-flake.url = "github:applicative-systems/mkdocs-flake";
  };

  outputs =
    inputs:
    let
      eachSystem =
        f:
        builtins.mapAttrs (
          system: pkgs': f system (pkgs'.extend inputs.self.overlays.default)
        ) inputs.nixpkgs.legacyPackages;

      inherit (inputs.nixpkgs) lib;
    in
    {
      overlays.default = import ./overlay.nix;

      packages = eachSystem (
        system: pkgs:
        {
          inherit (pkgs) nxbd;
          default = pkgs.nxbd;
        }
        // pkgs.lib.optionalAttrs (inputs.mkdocs-flake.packages ? ${system}) {
          documentation = pkgs.runCommand "nxbd-documentation" { nativeBuildInputs = [ pkgs.nxbd ]; } ''
            cp -r ${./documentation}/* .
            chmod -R u+w .
            nxbd generate-docs docs
            ${inputs.mkdocs-flake.packages.${system}.mkdocs}/bin/mkdocs build --site-dir $out
          '';
        }
      );

      devShells = eachSystem (
        system: pkgs: {
          default = pkgs.mkShell {
            inputsFrom = [ pkgs.nxbd ];

            packages = [
              pkgs.cargo
              pkgs.cargo-edit
              pkgs.cargo-watch
              pkgs.clippy
              pkgs.rust-analyzer
              pkgs.rustc
              pkgs.rustfmt
              inputs.self.formatter.${system}
            ];

            RUST_SRC_PATH = pkgs.rust.packages.stable.rustPlatform.rustLibSrc;
            RUST_BACKTRACE = 1;
          };
        }
      );

      formatter = eachSystem (
        _system: pkgs:
        pkgs.treefmt.withConfig {
          settings = {
            tree-root-file = "flake.nix";
            on-unmatched = "info";
            formatter = {
              nixfmt = {
                command = lib.getExe pkgs.nixfmt;
                includes = [ "*.nix" ];
              };
              statix = {
                command = lib.getExe pkgs.statix;
                options = [ "fix" ];
                no-positional-arg-support = true;
                includes = [ "*.nix" ];
              };
              deadnix = {
                command = lib.getExe pkgs.deadnix;
                options = [ "--edit" ];
                includes = [ "*.nix" ];
              };
              rustfmt = {
                command = lib.getExe pkgs.rustfmt;
                options = [
                  "--config"
                  "skip_children=true"
                  "--edition"
                  "2024"
                ];
                includes = [ "*.rs" ];
              };
              taplo = {
                command = lib.getExe pkgs.taplo;
                options = [ "format" ];
                includes = [ "*.toml" ];
              };
              # No markdown formatter: mkdocs wants 4 spaces indent for
              # multi level bullet point lists, which formatters break.
              shfmt = {
                command = lib.getExe pkgs.shfmt;
                options = [
                  "-w"
                  "-i"
                  "2"
                  "-s"
                ];
                includes = [
                  "*.sh"
                  "*.bash"
                  "*.envrc"
                ];
              };
            };
          };
        }
      );

      checks = eachSystem (
        system: pkgs:
        inputs.self.packages.${system}
        // pkgs.lib.optionalAttrs (pkgs.stdenv.hostPlatform.isx86_64 && pkgs.stdenv.hostPlatform.isLinux) {
          # Should run on all CPUs, but first we need to make the system
          # attribute inside the config a bit more dynamic.
          switch-local = pkgs.testers.runNixOSTest ./tests/switch-local.nix;
          switch-remote = pkgs.testers.runNixOSTest ./tests/switch-remote.nix;
        }
        // {
          formatting = inputs.self.formatter.${system}.check inputs.self;
        }
      );
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

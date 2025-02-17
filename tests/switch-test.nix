{ pkgs, ... }:

let
  inherit ((pkgs.nixos [ ./project-folder/configuration.nix ]).config.system.build) toplevel;
in

{
  name = "nxbd-switch-test";

  node.pkgsReadOnly = false;
  imports = [
    ./debug-vm.nix
  ];
  nodes = {
    machine =
      {
        lib,
        pkgs,
        modulesPath,
        ...
      }:
      {
        imports = [
          (modulesPath + "/profiles/installation-device.nix")
          (modulesPath + "/profiles/base.nix")
        ];

        nix = {
          settings = {
            substituters = lib.mkForce [ ];
            hashed-mirrors = null;
            connect-timeout = 1;
          };
          extraOptions = ''
            experimental-features = nix-command flakes
          '';
        };

        #system.includeBuildDependencies = true;

        virtualisation = {
          cores = 2;
          memorySize = 8000; # went OOM with lower values
        };

        virtualisation.useBootLoader = true;
        virtualisation.writableStore = true;
        virtualisation.additionalPaths = [
          toplevel
          toplevel.drvPath
          ./project-folder
          pkgs.path
        ];

        nixpkgs.overlays = [
          (import ../overlay.nix)
        ];

        environment.systemPackages = [
          pkgs.nxbd
        ];
      };
  };

  testScript = ''

    machine.start()
    machine.succeed("udevadm settle")
    machine.wait_for_unit("multi-user.target")

    machine.succeed("ls ${toplevel}")
    machine.succeed("cp ${./project-folder}/* .")

    machine.succeed("nix flake lock --override-input nixpkgs ${pkgs.path}")
    print(machine.succeed("cat flake.lock"))
    machine.succeed("nix -L build .#nixosConfigurations.machine.config.system.build.toplevel")

    machine.succeed("nxbd check")
    machine.succeed("nxbd build")
    machine.succeed("nxbd switch-local")

  '';
}

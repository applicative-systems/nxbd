{ pkgs, ... }:
{
  name = "nxbd-switch-test";

  node.pkgsReadOnly = false;

  nodes = {
    machine = import ./vm-config.nix;
  };

  testScript = ''
    machine.start()
    machine.succeed("udevadm settle")
    machine.wait_for_unit("multi-user.target")

    machine.succeed("ls ${toplevel}")
    machine.succeed("cp ${./project-folder}/* .")

    machine.succeed("nix flake lock --override-input nixpkgs ${pkgs.path}")

    # let's build this with nix before we build it with nxbd again.
    # in case it fails, we know if the config is broken or nxbd.
    machine.succeed("nix -L build .#nixosConfigurations.machine.config.system.build.toplevel")

    # TODO: Actually make check return a nonzero return code.
    # this way we can test if the ignore list works well.
    machine.succeed("nxbd check")
    machine.succeed("nxbd build")
    machine.succeed("nxbd switch-local")
  '';
}

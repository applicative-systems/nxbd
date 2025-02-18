{ pkgs, ... }:
{
  name = "nxbd-switch-local";

  node.pkgsReadOnly = false;

  nodes = {
    server = import ./vm-config.nix;
  };

  testScript = ''
    server.start()
    server.succeed("udevadm settle")
    server.wait_for_unit("multi-user.target")

    server.succeed("cp ${./project-folder}/* .")

    server.succeed("nix flake lock --override-input nixpkgs ${pkgs.path}")

    # let's build this with nix before we build it with nxbd again.
    # in case it fails, we know if the config is broken or nxbd.
    server.succeed("nix -L build .#nixosConfigurations.server.config.system.build.toplevel")

    # TODO: Actually make check return a nonzero return code.
    # this way we can test if the ignore list works well.
    server.succeed("nxbd check")
    server.succeed("nxbd build")
    server.succeed("nxbd switch-local")
  '';
}

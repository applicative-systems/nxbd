{ pkgs, ... }:

let
  sshConfig = builtins.toFile "ssh.conf" ''
    UserKnownHostsFile=/dev/null
    StrictHostKeyChecking=no
  '';

  inherit (import (pkgs.path + "/nixos/tests/ssh-keys.nix") pkgs) snakeOilPrivateKey;
in

{
  name = "nxbd-switch-test";

  node.pkgsReadOnly = false;

  nodes = {
    server =
      { lib, ... }:
      {
        imports = [ ./vm-config.nix ];
        networking.hostName = lib.mkForce "server";
      };
    deployer = import ./vm-config.nix;
  };

  interactive.nodes.server = import ./debug-vm.nix 2222;
  interactive.nodes.deployer = import ./debug-vm.nix 2223;

  testScript =
    { nodes, ... }:
    let
      targetNetworkJSON = pkgs.writeText "target-network.json" (
        builtins.toJSON nodes.server.system.build.networkConfig
      );

    in
    ''
      start_all()
      deployer.wait_for_unit("multi-user.target")

      deployer.succeed("mkdir /root/.ssh")
      deployer.copy_from_host("${snakeOilPrivateKey}", "/root/.ssh/id_ecdsa")
      deployer.succeed("chmod 600 /root/.ssh/id_ecdsa")
      deployer.copy_from_host("${sshConfig}", "/root/.ssh/config")
      deployer.succeed("cat ~/.ssh/config")

      # Prepare the project flake to build in the offline sandbox
      deployer.succeed("cp ${./project-folder}/* .")
      deployer.copy_from_host("${targetNetworkJSON}", "target-network.json")
      # Network of the initial VM needs to be restored so we're not offline
      # after the system switch
      deployer.succeed("sed -i 's@# EXTRA_IMPORTS@(lib.modules.importJSON ./target-network.json)@' configuration.nix")
      # Linter keeps removing the `lib` parameter
      deployer.succeed("sed -i 's@modulesPath, @lib, modulesPath, @' configuration.nix")
      deployer.succeed("grep -q target-network.json configuration.nix")
      deployer.succeed("nix flake lock --override-input nixpkgs ${pkgs.path}")

      server.wait_for_unit("multi-user.target")
      deployer.wait_until_succeeds("ping -c1 server")
      deployer.succeed("ssh -v -o ConnectTimeout=1 -o ConnectionAttempts=1 server echo hello")

      # let's build this with nix before we build it with nxbd again.
      # in case it fails, we know if the config is broken or nxbd.
      deployer.succeed("nix -L build .#nixosConfigurations.server.config.system.build.toplevel")

      deployer.succeed("nxbd check")
      deployer.succeed("nxbd build")
      deployer.succeed("nxbd switch-remote")
    '';
}

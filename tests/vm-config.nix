{
  lib,
  pkgs,
  modulesPath,
  ...
}:

let
  inherit ((pkgs.nixos [ ./project-folder/configuration.nix ]).config.system.build) toplevel;

  inherit (import (pkgs.path + "/nixos/tests/ssh-keys.nix") pkgs)
    snakeOilPrivateKey
    snakeOilPublicKey
    ;
in

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

  virtualisation = {
    cores = 2;
    memorySize = 8000; # went OOM with lower values
  };

  system.build.privateKey = snakeOilPrivateKey;
  system.build.publicKey = snakeOilPublicKey;
  users.users.root.openssh.authorizedKeys.keys = [ snakeOilPublicKey ];

  #virtualisation.useBootLoader = true;
  #virtualisation.writableStore = true;
  #virtualisation.moutHostNixStore = true;
  virtualisation.additionalPaths = [
    toplevel
    toplevel.drvPath
    ./project-folder
    pkgs.path
    snakeOilPrivateKey
  ];

  networking.useNetworkd = true;

  nixpkgs.overlays = [
    (import ../overlay.nix)
  ];

  environment.systemPackages = [
    pkgs.nxbd
  ];

  system.switch.enable = true;
}

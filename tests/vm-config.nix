{
  lib,
  pkgs,
  modulesPath,
  ...
}:

let
  inherit ((pkgs.nixos [ ./project-folder/configuration.nix ]).config.system.build) toplevel;
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
}

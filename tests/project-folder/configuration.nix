{ modulesPath, ... }:
{
  imports = [
    (modulesPath + "/testing/test-instrumentation.nix")
    ./hardware-configuration.nix
  ];

  boot.loader.grub = {
    enable = true;
    device = "/dev/vda";
  };

  documentation.enable = false;
  networking.hostName = "machine";
}

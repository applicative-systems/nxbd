{ modulesPath, ... }:
{
  imports = [
    (modulesPath + "/profiles/installation-device.nix")
    (modulesPath + "/profiles/base.nix")
    (modulesPath + "/testing/test-instrumentation.nix")
  ];

  boot.loader.grub = {
    enable = true;
    device = "/dev/vda";
  };

  documentation.enable = false;

  fileSystems."/" = {
    device = "/dev/vda1";
    fsType = "ext4";
  };

  nixpkgs.hostPlatform = "x86_64-linux";
}

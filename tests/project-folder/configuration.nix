{ lib, modulesPath, ... }:
{
  imports = [
    (modulesPath + "/profiles/qemu-guest.nix")
    (modulesPath + "/testing/test-instrumentation.nix")
    (modulesPath + "/virtualisation/qemu-vm.nix")
    # EXTRA_IMPORTS
  ];

  boot.loader.grub = {
    enable = true;
    device = "/dev/vda";
    forceInstall = true;
  };

  networking.useNetworkd = true;

  documentation.enable = false;
  networking.hostName = "server";
  services.openssh.enable = true;

  nixpkgs.hostPlatform = "x86_64-linux";
}

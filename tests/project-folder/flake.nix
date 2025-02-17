{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
  };

  outputs = inputs: {
    nixosConfigurations.machine = inputs.nixpkgs.lib.nixosSystem {
      modules = [ ./configuration.nix ];
    };
  };
}

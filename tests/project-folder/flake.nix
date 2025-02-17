{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
  };

  outputs = inputs: {
    nixosConfigurations.test = inputs.nixpkgs.lib.nixosSystem {
      modules = [ ./configuration.nix ];
    };
  };
}

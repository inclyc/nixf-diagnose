{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    flake-parts.url = "github:hercules-ci/flake-parts";
  };
  outputs =
    {
      flake-parts,
      nixpkgs,
      ...
    }@inputs:
    flake-parts.lib.mkFlake { inherit inputs; } {

      perSystem =
        { pkgs, ... }:
        {
          packages.default = pkgs.callPackage ./default.nix { };
        };

      systems = nixpkgs.lib.systems.flakeExposed;
    };
}

{
  description = "Nix garbage collector root flake updater";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils, ... }: flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      # The default package
      packages.default = pkgs.rustPlatform.buildRustPackage rec {
        pname = "nixpkgsupd";
        version = "0.1.0";

        src = ./.;

        cargoHash = "sha256-8e2n6Ys3VjJVQr5DObTG4HireviSQlX0q4JuJG/+XZ0=";
      };

      devShells.default = pkgs.mkShell {
        buildInputs = [ pkgs.rustc pkgs.cargo ];
      };
    });
}


{
  description = "Nix garbage collector root flake updater";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils, ... }: flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      packages.default = pkgs.rustPlatform.buildRustPackage {
        pname = "nixpkgsupd";
        version = "0.2.0";

        src = ./.;

        cargoLock.lockFile = ./Cargo.lock;
      };

      devShells.default = pkgs.mkShell {
        buildInputs = [ pkgs.rustc pkgs.cargo ];
      };
    });
}


{
  description = "hstdb";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
  inputs.flake-utils.url = "github:numtide/flake-utils";

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        rustPlatform = pkgs.rustPlatform;
      in
      {
        packages.default = rustPlatform.buildRustPackage {
          pname = "hstdb";
          version = "3.0.0";

          src = ./.;
          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          buildInputs = [ pkgs.sqlite ];
          nativeBuildInputs = [ pkgs.pkg-config ];
        };

        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/hstdb";
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [ pkgs.sqlite ];

          nativeBuildInputs = with pkgs; [
            rustc
            cargo
            pkg-config
          ];
        };
      }
    );
}

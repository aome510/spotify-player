{
  description = "spotify-player flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        defaultPackage = pkgs.callPackage ./default.nix { };
        devShell =
          with pkgs;
          mkShell {
            buildInputs = [
              cargo
              pkg-config
              rustc

              # spotify-player dependencies
              alsa-lib
              dbus-glib
              libsixel
              openssl
            ];
          };
      }
    );
}

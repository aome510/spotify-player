let
  # Pinned nixpkgs, deterministic. Last updated: 2025-02-28
  pkgs = import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/4f2a999cd412fa5a231e487a11b2c50677ff595c.tar.gz") {};
  # Rolling updates, not deterministic.
  # pkgs = import (fetchTarball("channel:nixpkgs-unstable")) {};
in
  pkgs.mkShell {
    buildInputs = [
      pkgs.alsa-lib
      pkgs.cargo
      pkgs.dbus-glib
      pkgs.libsixel
      pkgs.openssl
      pkgs.pkg-config
      pkgs.rustc
    ];
  }

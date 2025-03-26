let
  # Pinned nixpkgs, deterministic. Last updated: 2025-02-28
  pkgs = import (fetchTarball("https://github.com/NixOS/nixpkgs/archive/f44bd8ca21e026135061a0a57dcf3d0775b67a49.tar.gz")) {};

  # Rolling updates, not deterministic.
  # pkgs = import (fetchTarball("channel:nixpkgs-unstable")) {};
in pkgs.mkShell {
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

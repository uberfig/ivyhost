{ pkgs ? import <nixpkgs> { } }:
let manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
in
pkgs.rustPlatform.buildRustPackage rec {
  pname = manifest.name;
  version = manifest.version;
  cargoLock.lockFile = ./Cargo.lock;
  src = pkgs.lib.cleanSource ./.;
  nativeBuildInputs = with pkgs; [
    libgit2
    openssl
    openssl.dev
    postgresql
    pkg-config
  ];
  buildInputs = with pkgs; [
    libgit2
    openssl
    openssl.dev
    postgresql
    pkg-config
  ];
}

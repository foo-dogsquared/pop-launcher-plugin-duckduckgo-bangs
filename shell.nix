{ pkgs ? import <nixpkgs> {} }:

with pkgs;

mkShell {
  buildInputs = [
    gnumake
    glibc
    clippy # Our favorite linter
    openssl # Needed for the reqwest library
    rustc
    rustfmt # Prettifier
    rust-analyzer # LSP server
  ];
}

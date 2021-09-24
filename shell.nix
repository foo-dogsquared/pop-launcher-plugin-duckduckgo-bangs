{ pkgs ? import <nixpkgs> {} }:

with pkgs;

mkShell {
  buildInputs = [
    gnumake
    glibc
    clippy
    rustc
    rustfmt
    rust-analyzer
  ];
}

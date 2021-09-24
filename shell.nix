{ pkgs ? import <nixpkgs> {} }:

with pkgs;

mkShell {
  buildInputs = [
    gnumake
    clippy
    rustc
    rustfmt
    rust-analyzer
  ];
}

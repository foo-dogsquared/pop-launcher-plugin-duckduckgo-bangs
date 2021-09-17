{ pkgs ? import <nixpkgs> {} }:

with pkgs;

mkShell {
  buildInputs = [
    gnumake
    rustc
    rustfmt
    rust-analyzer
  ];
}

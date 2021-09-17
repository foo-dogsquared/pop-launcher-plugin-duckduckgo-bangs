{ pkgs ? import <nixpkgs> {} }:

with pkgs;

mkShell {
  buildInputs = [
    gnumake
    rustc
    rust-analyzer
  ];
}

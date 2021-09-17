{
  description = "A very basic flake";
  inputs.nixpkgs.url = github:NixOS/nixpkgs/21.05;

  outputs = { self, nixpkgs }: {
    defaultPackage.x86_64-linux =
      with import nixpkgs { system = "x86_64-linux"; };
      rustPlatform.buildRustPackage {
        name = "pop-shell-launcher-bangs";
        src = self;
        cargoSha256 = lib.fakeSha256;
      };
  };
}

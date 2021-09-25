{
  description = "A very basic flake";
  inputs.nixpkgs.url = github:NixOS/nixpkgs/21.05;

  outputs = { self, nixpkgs }: {
    defaultPackage.x86_64-linux =
      with import nixpkgs { system = "x86_64-linux"; };
      rustPlatform.buildRustPackage {
        name = "pop-launcher-plugin-bangs";
        src = self;
        cargoSha256 = "sha256-achaZ7QE7cHpXZ2lzt3G5Ja3PnKGqj+t9up/q3FeHxc=";
      };
  };
}

{
  description = "Prometheus Weather exporter";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    darwin.url = "github:lnl7/nix-darwin/master";
    darwin.inputs.nixpkgs.follows = "nixpkgs";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, darwin, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        nightly = pkgs.rust-bin.selectLatestNightlyWith (toolchain:
          toolchain.default.override {
            extensions = [ "rust-src" "rust-analyzer" "rust-std" ];
            targets = [ "aarch64-apple-darwin" "x86_64-unknown-linux-gnu" ];
          });
      in {
        devShell = pkgs.mkShell {
          packages = [ nightly pkgs.darwin.apple_sdk.frameworks.Security pkgs.openssl pkgs.pkgconfig ];
        };
      });
}

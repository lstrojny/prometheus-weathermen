{
  description = "Prometheus Weather exporter";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rust-toolchain = pkgs.symlinkJoin {
          name = "rust-toolchain";
          paths = [ pkgs.rust-bin.beta.latest.default ];
        };
      in { devShell = pkgs.mkShell { packages = [ rust-toolchain ]; }; });
}

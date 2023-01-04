{
  description = "Prometheus Weather exporter";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { system = system; };
        rust-toolchain = pkgs.symlinkJoin {
          name = "rust-toolchain";
          paths = [ pkgs.rustc pkgs.cargo pkgs.rustfmt pkgs.rustPlatform.rustcSrc ];
        };
      in { devShell = pkgs.mkShell { packages = [ rust-toolchain ]; }; });
}

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
        rust-env = pkgs.rust-bin.stable.latest.complete;
      in {
        devShell = pkgs.mkShell {
          packages = [
            pkgs.rust-bin.stable.latest.complete
            pkgs.darwin.apple_sdk.frameworks.Security
            pkgs.openssl
            pkgs.pkgconfig
          ];
          shellHook = ''
            echo "Toolchain: ${pkgs.lib.getBin rust-env}/bin"
            echo " rust-std: ${pkgs.lib.getLib rust-env}/lib/rustlib/src/rust/library"
          '';
        };
      });
}

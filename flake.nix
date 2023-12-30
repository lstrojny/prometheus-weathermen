{
  description = "prometheus-weathermen";

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
        cargo-unused-imports = pkgs.rustPlatform.buildRustPackage rec {
          pname = "cargo-unused-features";
          version = "0.2.0";

          src = pkgs.fetchFromGitHub {
            owner = "TimonPost";
            repo = pname;
            rev = version;
            hash = "sha256-wpu55tqw41lSpEZu94s9UEwf7Oq0ar5Fhh9ApkhaBtE=";
          };

          cargoHash = "sha256-K9I7Eg43BS2SKq5zZ3eZrMkmuHAx09OX240sH0eGs+k=";

          buildInputs = [
            pkgs.openssl
            pkgs.darwin.apple_sdk.frameworks.Security
            pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
          ];

          meta = {
            description = "Potential unused, enabled feature flag finder and pruner";
            homepage = "https://github.com/TimonPost/cargo-unused-features";
          };
        };
        rust-env = pkgs.rust-bin.selectLatestNightlyWith
          (toolchain: toolchain.default.override { extensions = [ "rust-src" ]; });
      in {
        devShell = pkgs.mkShell {
          packages = [
            rust-env
            cargo-unused-imports
            pkgs.darwin.apple_sdk.frameworks.Security
            pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
            pkgs.darwin.apple_sdk.frameworks.CoreServices
            pkgs.openssl
            pkgs.pkg-config
          ];
          shellHook = ''
            echo "Toolchain: ${pkgs.lib.getBin rust-env}/bin"
            echo " rust-std: ${pkgs.lib.getLib rust-env}/lib/rustlib/src/rust/library"
          '';
        };
      });
}

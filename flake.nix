{
  description = "purecrate-ts development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        # Pinned rather than `stable.latest`: panic messages that the
        # differential tests compare (e.g. char::to_digit) were measured on
        # this release.
        rust = pkgs.rust-bin.stable."1.98.1".default.override {
          extensions = [ "rust-src" "rust-analyzer" "clippy" "rustfmt" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [
            rust
            # parseJson needs JSON.parse source text access (Node 21+); CI
            # uses 24. TypeScript 6/7 and the schema libraries come from npm.
            pkgs.nodejs_24
          ];
        };
      });
}

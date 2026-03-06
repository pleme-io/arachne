{
  description = "Arachne - Generic classifieds scraping framework";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    substrate = {
      url = "github:pleme-io/substrate";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
    };
    crate2nix = {
      url = "github:nix-community/crate2nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    devenv = {
      url = "github:cachix/devenv";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, substrate, crate2nix, ... }: let
    systems = ["aarch64-darwin" "x86_64-linux" "aarch64-linux"];
    eachSystem = f: nixpkgs.lib.genAttrs systems f;
  in {
    # Dev shells for working on the library
    devShells = eachSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ substrate.rustOverlays.${system}.rust ];
      };
    in {
      default = pkgs.mkShell {
        buildInputs = with pkgs; [
          fenixRustToolchain
          rust-analyzer
          cargo-watch
          openssl
          postgresql
          pkg-config
          cmake
          perl
          crate2nix.packages.${system}.default
        ];
        RUST_SRC_PATH = "${pkgs.fenixRustToolchain}/lib/rustlib/src/rust/library";
      };
    });

    # Generic home-manager module — requires package override from plugins overlay
    homeManagerModules.default = import ./module {
      hmHelpers = import "${substrate}/lib/hm-service-helpers.nix" { lib = nixpkgs.lib; };
    };

    # Generic NixOS module — requires package override from plugins overlay
    nixosModules.default = import ./module/nixos.nix;
  };
}

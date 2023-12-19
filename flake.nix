{
  description = "Build a cargo project without extra checks";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";

    nix-vscode-extensions = {
      url = "github:nix-community/nix-vscode-extensions";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = {
    self,
    nixpkgs,
    crane,
    flake-utils,
    fenix,
    nix-vscode-extensions,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        config.allowUnfree = true;
      };

      toolchain = with fenix.packages.${system};
        combine [
          minimal.rustc
          minimal.cargo
          complete.rust-src
          targets.thumbv7em-none-eabihf.latest.rust-std
        ];

      craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;

      my-crate = craneLib.buildPackage {
        src = craneLib.cleanCargoSource (craneLib.path ./.);
        strictDeps = true;

        cargoExtraArgs = "--target thumbv7em-none-eabihf";
        doCheck = false;

        buildInputs =
          [
            # Add additional build inputs here
          ]
          ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            # Additional darwin specific inputs can be set here
            pkgs.libiconv
            pkgs.darwin.IOKit
          ];
      };
    in {
      checks = {
        inherit my-crate;
      };

      packages.default = my-crate;

      apps.default = flake-utils.lib.mkApp {
        drv = my-crate;
      };

      devShells.default = craneLib.devShell {
        # Inherit inputs from checks.
        checks = self.checks.${system};

        # Additional dev-shell environment variables can be set directly
        # MY_CUSTOM_DEVELOPMENT_VAR = "something else";
        RUST_SRC_PATH = "${fenix.packages.${system}.complete.rust-src}/lib/rustlib/src/rust/library";
        # Extra inputs can be added here; cargo and rustc are provided by default.
        packages = with pkgs; [
          darwin.IOKit
          probe-rs
        ];
      };
    });
}

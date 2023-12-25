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
  };

  outputs = {
    self,
    nixpkgs,
    crane,
    flake-utils,
    fenix,
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

      my-crate = let
        # Only keeps markdown files
        linkerFilter = path: _type: builtins.match ".*x$" path != null;
        linkerOrCargo = path: type:
          (linkerFilter path type) || (craneLib.filterCargoSources path type);
      in
        craneLib.buildPackage
        {
          src = pkgs.lib.cleanSourceWith {
            src = craneLib.path ./.;
            filter = linkerOrCargo;
          };
          strictDeps = true;

          cargoExtraArgs = "--target thumbv7em-none-eabihf";
          doCheck = false;

          buildInputs =
            [
              # Add additional build inputs here
              pkgs.SDL2
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              # Additional darwin specific inputs can be set here
              pkgs.libiconv
              pkgs.darwin.IOKit
            ];

          extraDummyScript = ''
            cp -a ${./memory.x} $out/memory.x
            rm -rf $out/src/bin/crane-dummy-*
          '';
        };
    in {
      # checks = {
      #   inherit my-crate;
      # };

      # packages.default = my-crate;

      # apps.default = flake-utils.lib.mkApp {
      #   drv = my-crate;
      # };

      devShells.default = craneLib.devShell {
        # checks = self.checks.${system};

        RUST_SRC_PATH = "${fenix.packages.${system}.complete.rust-src}/lib/rustlib/src/rust/library";

        packages = with pkgs; [
          pkgs.SDL2
          darwin.IOKit
          pkgs.libiconv
          probe-rs
          rust-analyzer
        ];
      };
    });
}

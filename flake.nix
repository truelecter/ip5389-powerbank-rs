{
  description = "Build a cargo project without extra checks";

  inputs = {
    nixpkgs.url = "nixpkgs";

    crane.url = "github:ipetkov/crane";

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
          default.clippy
          default.rustfmt
          complete.rust-src
          targets.thumbv7em-none-eabihf.latest.rust-std
        ];

      craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;

      linkerFilter = path: _type: builtins.match ".*x$" path != null;
      linkerOrCargo = path: type:
        (linkerFilter path type) || (craneLib.filterCargoSources path type);

      firmwareArgs = {
        inherit (craneLib.crateNameFromCargoToml {cargoToml = ./firmware/Cargo.toml;}) pname version;

        src = pkgs.lib.cleanSourceWith {
          src = craneLib.path ./.;
          filter = linkerOrCargo;
        };
        strictDeps = true;

        cargoExtraArgs = "--target thumbv7em-none-eabihf";
        doCheck = false;

        buildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [
          # Additional darwin specific inputs can be set here
          pkgs.libiconv
        ];

        extraDummyScript = ''
          cp -a ${./memory.x} $out/memory.x
        '';
      };

      firmware = craneLib.buildPackage (firmwareArgs
        // {
          # The dummy binaries crane builds dependencies with don't link against cortex-m-rt's
          # linker script, so only build libraries in that step
          cargoArtifacts = craneLib.buildDepsOnly (firmwareArgs
            // {
              cargoExtraArgs = "${firmwareArgs.cargoExtraArgs} --lib";
            });
        });

      emulator =
        craneLib.buildPackage
        {
          inherit (craneLib.crateNameFromCargoToml {cargoToml = ./emulation/Cargo.toml;}) pname version;

          src = pkgs.lib.cleanSourceWith {
            src = craneLib.path ./.;
            filter = linkerOrCargo;
          };
          strictDeps = true;
          # emulation is its own workspace with its own lock file
          cargoVendorDir = craneLib.vendorMultipleCargoDeps {
            cargoLockList = [./Cargo.lock ./emulation/Cargo.lock];
          };
          buildInputs =
            [
              # Add additional build inputs here
              pkgs.SDL2
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [
              # Additional darwin specific inputs can be set here
              pkgs.libiconv
            ];

          doCheck = false;

          # emulation is its own workspace (and builds for the host). Crane's dependency-only
          # step doesn't carry its lock file over, so build everything in one step.
          cargoArtifacts = null;
          cargoBuildCommand = "cargoWithProfile build --manifest-path emulation/Cargo.toml";
          # Crane only installs binaries of the workspace `cargo metadata` reports from the
          # current directory
          installPhaseCommand = ''
            log="$(realpath "$cargoBuildLog")"
            (cd emulation && installFromCargoBuildLog "$out" "$log")
          '';
        };
    in rec {
      checks = {
        inherit firmware;
      };

      packages = {
        inherit emulator firmware;

        default = firmware;
      };

      apps.default = flake-utils.lib.mkApp {
        drv = emulator;
      };

      devShells.default = craneLib.devShell {
        checks = self.checks.${system};

        RUST_SRC_PATH = "${fenix.packages.${system}.complete.rust-src}/lib/rustlib/src/rust/library";

        packages = with pkgs; [
          pkgs.SDL2
          pkgs.libiconv
          probe-rs-tools
          rust-analyzer
          gcc-arm-embedded
        ];
      };

      devShell = devShells.default;
    });
}

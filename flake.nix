{
  description = "Sigfinn - lifecycle manager for spawning tasks";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    inputs@{
      self,
      nixpkgs,
      flake-parts,
      fenix,
      crane,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {

      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      flake = {
        overlays.default = final: prev: { };
      };

      perSystem =
        {
          config,
          self',
          inputs',
          pkgs,
          system,
          ...
        }:
        let

          pkgs = import nixpkgs {
            inherit system;
            overlays = [
              self.overlays.default
              fenix.overlays.default
            ];
          };

          cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
          name = cargoToml.workspace.metadata.crane.name;
          version = cargoToml.workspace.package.version;

          rustToolchain =
            with fenix.packages.${system};
            combine [
              stable.rustc
              stable.cargo
              stable.clippy
              stable.rust-src
              stable.rust-std
              targets.x86_64-unknown-linux-musl.stable.rust-std
              targets.aarch64-unknown-linux-musl.stable.rust-std
              default.rustfmt
            ];

          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };

          rustPlatformMusl = pkgs.pkgsStatic.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };

          isCross = system == "x86_64-linux";
          isCrossFromAarch64 = system == "aarch64-linux";

          crossPkgs =
            if isCross then
              import nixpkgs {
                inherit system;
                crossSystem = {
                  config = "aarch64-unknown-linux-musl";
                };
                overlays = [
                  self.overlays.default
                  fenix.overlays.default
                ];
              }
            else if isCrossFromAarch64 then
              import nixpkgs {
                inherit system;
                crossSystem = {
                  config = "x86_64-unknown-linux-musl";
                };
                overlays = [
                  self.overlays.default
                  fenix.overlays.default
                ];
              }
            else
              null;

          rustPlatformCrossMusl =
            if isCross || isCrossFromAarch64 then
              crossPkgs.pkgsStatic.makeRustPlatform {
                cargo = rustToolchain;
                rustc = rustToolchain;
              }
            else
              null;

          cargoArgs = [
            "--workspace"
            "--bins"
            "--examples"
            "--tests"
            "--benches"
            "--all-targets"
          ];
          unitTestArgs = [ "--workspace" ];
        in
        {

          formatter = pkgs.treefmt;

          devShells.default = pkgs.callPackage ./devshell {
            inherit
              rustToolchain
              cargoArgs
              unitTestArgs
              ;
          };

          packages = {
            check-format = pkgs.callPackage ./devshell/format.nix { };
          };
        };
    };
}

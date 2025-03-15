{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

    advisory-db.url = "github:rustsec/advisory-db";
    advisory-db.flake = false;

    crane.url = "github:ipetkov/crane";

    treefmt.url = "github:numtide/treefmt-nix";
    treefmt.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = {
    self,
    nixpkgs,
    advisory-db,
    crane,
    treefmt,
  }: let
    forEachSystem = nixpkgs.lib.genAttrs [
      "aarch64-linux"
      "x86_64-linux"
    ];

    mkCommonArgs = pkgs: {
      src = (crane.mkLib pkgs).cleanCargoSource self;

      RUSTC_BOOTSTRAP = 1;

      nativeBuildInputs = with pkgs; [
        pkg-config
      ];

      buildInputs = with pkgs; [
        openssl
      ];

      strictDeps = true;

      preCheck = ''
        export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
      '';

      meta = {
        repository = "https://github.com/newAM/cfddns";
        license = [nixpkgs.lib.licenses.mit];
        maintainers = [nixpkgs.lib.maintainers.newam];
        mainProgram = "cfddns";
      };
    };

    mkCargoArtifacts = pkgs: (crane.mkLib pkgs).buildDepsOnly (mkCommonArgs pkgs);

    treefmtEval = pkgs:
      treefmt.lib.evalModule pkgs {
        projectRootFile = "flake.nix";
        programs = {
          alejandra.enable = true;
          prettier.enable = true;
          rustfmt = {
            enable = true;
            edition = (nixpkgs.lib.importTOML ./Cargo.toml).package.edition;
          };
          taplo.enable = true;
        };
      };
  in {
    devShells = forEachSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
        commonArgs = mkCommonArgs pkgs;
      in {
        default = pkgs.mkShell {
          inherit (commonArgs) nativeBuildInputs buildInputs;

          shellHook = let
            libPath = nixpkgs.lib.makeLibraryPath commonArgs.buildInputs;
          in ''
            export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig"
            export LD_LIBRARY_PATH="${libPath}";
          '';
        };
      }
    );

    packages = forEachSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
      in {
        default = (crane.mkLib pkgs).buildPackage (
          nixpkgs.lib.recursiveUpdate (mkCommonArgs pkgs) {
            cargoArtifacts = mkCargoArtifacts pkgs;
          }
        );
      }
    );

    formatter = forEachSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
      in
        (treefmtEval pkgs).config.build.wrapper
    );

    checks = forEachSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
        commonArgs = mkCommonArgs pkgs;
      in {
        pkg = self.packages.${system}.default;

        formatting = (treefmtEval pkgs).config.build.check self;

        audit = (crane.mkLib pkgs).cargoAudit (
          nixpkgs.lib.recursiveUpdate commonArgs {
            inherit advisory-db;
          }
        );

        clippy = (crane.mkLib pkgs).cargoClippy (
          nixpkgs.lib.recursiveUpdate commonArgs {
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            cargoArtifacts = mkCargoArtifacts pkgs;
          }
        );
      }
    );

    overlays.default = final: prev: {
      cfddns = self.packages.${prev.system}.default;
    };

    nixosModules.default = import ./nixos/module.nix;
  };
}

{
  nixConfig.bash-prompt-prefix = ''(BMM) '';

  inputs = {
    gitignore = {
      url = "github:hercules-ci/gitignore.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    flake-compat.url = "https://flakehub.com/f/edolstra/flake-compat/1.tar.gz";

    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    gitignore,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};

        drv = let
          cargo-toml = pkgs.lib.importTOML ./src-tauri/Cargo.toml;

          src = pkgs.lib.cleanSourceWith {
            name = "${pname}-${version}-clean-src";
            src = ./.;
            filter = gitignore.lib.gitignoreFilterWith {
              basePath = ./.;
              extraRules = "/.hack\n" + builtins.readFile ./.dockerignore;
            };
          };
          pname = cargo-toml.package.name;
          version = cargo-toml.package.version;

          mainProgram =
            # fallback to null in order to crash if no main binary found
            (pkgs.lib.lists.findFirst (f: f.path == "src/main.rs") null cargo-toml.bin).name;
        in
          pkgs.rustPlatform.buildRustPackage {
            inherit src pname version;
            doCheck = false;
            patchPhase = ''
              # allow pnpm, upstreamed to nixpkgs master
              cp ${pkgs.writeText "${pname}-${version}-package.json" (
                builtins.toJSON (
                  (pkgs.lib.importJSON "${src}/package.json") // {packageManager = null;}
                )
              )} package.json

              cp ${./.hack/pnpm-lock.yaml} pnpm-lock.yaml
            '';

            buildAndTestSubdir = "src-tauri";
            cargoRoot = "src-tauri";
            cargoLock.lockFile = "${src}/src-tauri/Cargo.lock";
            cargoLock.outputHashes = {
              "fix-path-env-0.0.0" = "sha256-SHJc86sbK2fA48vkVjUpvC5FQoBOno3ylUV5J1b4dAk=";
            };

            # using pnpm to fetch deps and bun to build, since nix doesn't have a bun fetcher
            pnpmDeps = pkgs.pnpm.fetchDeps {
              inherit src pname version;
              hash = "sha256-faofv1ReGaS1sHiHHBrN/2QA8aPhxj01ZtSjkBALs9E=";
            };

            nativeBuildInputs = with pkgs; [
              pkg-config
              bun
              pnpm.configHook
              cargo-tauri.hook
              wrapGAppsHook3
            ];

            buildInputs = with pkgs;
              [openssl]
              ++ lib.optionals stdenv.isLinux [
                atk
                cairo
                gdk-pixbuf
                glib
                gtk3
                harfbuzz
                librsvg
                libsoup_3
                pango
                webkitgtk_4_1
              ]
              ++ lib.optionals stdenv.isDarwin [darwin.apple_sdk.frameworks.WebKit];

            postInstall = with pkgs;
              lib.optionalString stdenv.hostPlatform.isDarwin ''
                mkdir -p "$out/bin"
                ln -sf "$out/Applications/${pname}.app/Contents/MacOS/${pname}" "$out/bin/${mainProgram}"
              '';

            meta = {
              inherit mainProgram;
              homepage = "https://balatro-mod-manager.dasguney.com";
              license = pkgs.lib.licenses.gpl3;
              platforms = with pkgs.lib.platforms; linux ++ darwin; # darwin support untested
            };
          };
      in {
        packages.default = self.packages.${system}.balatro-mod-manager;
        packages.balatro-mod-manager = drv;

        devShells.default = self.devShells.${system}.pure;
        devShells.pure = pkgs.mkShell {
          inputsFrom = [self.packages.${system}.balatro-mod-manager];
          packages = with pkgs;
            lib.optionals stdenv.isLinux [xdg-utils]
            ++ [
              go-task
              rust-analyzer
              clippy
              rustfmt
            ];
          shellHook = with pkgs;
            lib.optionalString stdenv.hostPlatform.isLinux ''
              export GSETTINGS_SCHEMA_DIR="${glib.getSchemaPath gtk3}"
            '';
        };
      }
    );
}

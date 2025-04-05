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

    mesa_24_0-pin.url = "github:nixos/nixpkgs/e913ae340076bbb73d9f4d3d065c2bca7caafb16";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    gitignore,
    ...
  } @ inputs:
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

          postPatch = ''
            # use the old package.json and allow `pnpm` by removing the `packageManager` field
            cp ${pkgs.writeText "${pname}-${version}-package.json" (
              builtins.toJSON (
                (pkgs.lib.importJSON ./.hack/package.json) // {packageManager = null;}
              )
            )} package.json
            cp ${./.hack/package-lock.json} package-lock.json
          '';

          mainProgram =
            # fallback to null in order to crash if no main binary found
            (pkgs.lib.lists.findFirst (f: f.path == "src/main.rs") null cargo-toml.bin).name;
        in
          pkgs.rustPlatform.buildRustPackage {
            inherit src pname version postPatch;
            doCheck = false;

            buildAndTestSubdir = "src-tauri";
            cargoRoot = "src-tauri";
            cargoLock.lockFile = "${src}/src-tauri/Cargo.lock";
            cargoLock.outputHashes = {
              "fix-path-env-0.0.0" = "sha256-SHJc86sbK2fA48vkVjUpvC5FQoBOno3ylUV5J1b4dAk=";
            };

            # using npm to fetch deps and bun to build, since nix doesn't have a bun fetcher
            npmDeps = pkgs.fetchNpmDeps {
              inherit src postPatch;
              name = "${pname}-${version}-npm-deps";
              hash = "sha256-jFyKuqmJsvFgSIyLM0PmiMz+5XlG/k+AOF+e5rYx498=";
            };

            nativeBuildInputs = with pkgs; [
              pkg-config
              bun
              cargo-tauri.hook

              nodejs
              npmHooks.npmConfigHook

              wrapGAppsHook3
            ];

            buildInputs = with pkgs;
              [openssl]
              ++ lib.optionals stdenv.isLinux (
                [inputs.mesa_24_0-pin.legacyPackages.${system}.webkitgtk_4_1]
                ++ [
                  atk
                  cairo
                  gdk-pixbuf
                  glib
                  gtk3
                  harfbuzz
                  librsvg
                  libsoup_3
                  pango
                ]
              )
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

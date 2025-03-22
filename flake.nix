{
  nixConfig.bash-prompt-prefix = ''(BMM) '';

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    nixpkgs,
    flake-utils,
    ...
  } @ inputs:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import inputs.rust-overlay)];
        pkgs = import nixpkgs {inherit system overlays;};
      in {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            bun
            go-task

            rust-analyzer
            rust-bin.stable.latest.default
            cargo-tauri
            pkg-config

            glib
            glib-networking
            gtk3
            librsvg
            openssl
            webkitgtk_4_1
          ];

          shellHook = ''
            export GSETTINGS_SCHEMA_DIR="${pkgs.glib.getSchemaPath pkgs.gtk3}"
          '';
        };
      }
    );
}

{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/25.11";
    flake-parts.url = "github:hercules-ci/flake-parts";
    systems.url = "github:nix-systems/default";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      systems = import inputs.systems;

      perSystem =
        {
          config,
          self',
          pkgs,
          lib,
          system,
          ...
        }:
        let
          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            extensions = [
              "rust-src"
              "rust-analyzer"
              "clippy"
            ];
          };

        in
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ inputs.rust-overlay.overlays.default ];
            config.allowUnfree = true;
          };

          devShells.default = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              bash
              vscode-extensions.vadimcn.vscode-lldb
              cmake
              shaderc
              llvmPackages.clang
              llvmPackages.libclang
              rustToolchain
            ];
            buildInputs = with pkgs; [
              vulkan-loader
              vulkan-headers
              cudatoolkit
            ];
            env = {
              CODELLDB_PATH = "${pkgs.vscode-extensions.vadimcn.vscode-lldb}/share/vscode/extensions/vadimcn.vscode-lldb/adapter/codelldb";
              LIBLLDB_PATH = "${pkgs.vscode-extensions.vadimcn.vscode-lldb}/share/vscode/extensions/vadimcn.vscode-lldb/lldb/lib/liblldb.so";
              LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
            };
            shellHook = ''
              export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath [ pkgs.vulkan-loader ]}:$LD_LIBRARY_PATH"
            '';
          };
        };
    };
}

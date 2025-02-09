{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = {
    nixpkgs,
    fenix,
    ...
  }: let
    system = "x86_64-linux";
    pkgs = import nixpkgs {
      inherit system;
    };
    lib = pkgs.lib;

    fenix-lib = fenix.packages.${system};

    rust-toolchain-toml = (lib.importTOML ./rust-toolchain.toml).toolchain;
    rust-toolchain = fenix-lib.fromToolchainName {
      name = rust-toolchain-toml.channel;
      sha256 = "sha256-yMuSb5eQPO/bHv+Bcf/US8LVMbf/G/0MSfiPwBhiPpk=";
    };

    libraries = with pkgs; [
      gtk3-x11.dev
      xorg.libXcursor
      xorg.libXrandr
      xorg.libXi
      libxkbcommon
    ];
  in {
    devShells.${system}.default = pkgs.mkShell {
      buildInputs = with pkgs;
        [
          nushell
          pkg-config
          openssl
          (rust-toolchain.withComponents (rust-toolchain-toml.components))
        ]
        ++ libraries;

      LD_LIBRARY_PATH = lib.makeLibraryPath libraries;
    };
  };
}

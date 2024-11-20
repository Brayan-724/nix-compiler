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

      fenix-lib = fenix.packages.${system};

      rust-toolchain = fenix-lib.fromToolchainFile {
        file = ./rust-toolchain.toml;
        sha256 = "sha256-yMuSb5eQPO/bHv+Bcf/US8LVMbf/G/0MSfiPwBhiPpk=";
      };
  in {

      devShells.${system}.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          nushell
          pkg-config
          openssl
          rust-toolchain
        ];
      };
  };
}

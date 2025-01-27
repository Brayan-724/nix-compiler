# let
#   nixpkgs = ./../nixpkgs;
#   system = "x86_64-linux";
#   pkgs = import nixpkgs {
#     inherit system;
#   };
#   out = pkgs.hello;
#   # out = pkgs.mkShell {
#   #   buildInputs = with pkgs; [
#   #     hello
#   #   ];
#   # };
# in out
########
# let
#   merged = let
#     some = {
#       inherit config;
#
#       options = {
#         precious = "TARGET";
#         other = 123;
#       };
#     };
#   in
#     builtins.inspect false some;
#
#   options = merged.options;
#
#   config = let
#     other_config = builtins.mapAttrs (_: v: v) options;
#   in
#     other_config.precious;
#
#   result = {
#     inherit config;
#   };
# in
#   result
########
let
  system = "x86_64-linux";
  pkgs = import ../nixpkgs {
    inherit system;
  };
in {
  devShells.${system}.default = pkgs.mkShell {
    buildInputs = with pkgs; [
      hello
    ];
  };
}

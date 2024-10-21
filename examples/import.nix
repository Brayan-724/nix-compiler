let
  hello-world = import ./minimal.nix;
in {
  message = hello-world.hello.world;
}

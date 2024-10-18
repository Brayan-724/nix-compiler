let 
  hello = rec {
    world = "World";
    exclamation = "!";

    text = "Hello ${world}${exclamation}";
  };
in {
  hello.world = hello.world.exclamation;
}

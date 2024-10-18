let 
  hello.world.exclamation = "Hello World!";

  hello = { # <-- world.exclamation
    world = {
      exclamation = "...";
    };
  };
in {
  hello.world = hello.world.exclamation;

  hello = {
    world = hello.world.exclamation;
  };
}

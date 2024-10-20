let
  phrases = {
    hello-world = "Hello World!";
  };
in with phrases; {
    hello-world = hello-world;
}

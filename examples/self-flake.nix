{
  outputs = { self }: let
    message = self.hello-world;
  in {
    hello-world = "Hello World!";

    inherit message;
  };
}

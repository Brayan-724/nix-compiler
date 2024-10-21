let 
  hello-world = "Hello World!";
  pkgs = {
    hello = "Hello World!";
  };
in {
  inherit hello-world;
  inherit (pkgs) hello;

}

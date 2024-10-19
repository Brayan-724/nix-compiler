# Minimal functionality of Nix
#
# This test just the chained objects.
# TODO: Is it called `chained objects`?
#
# The expected output is:
#@@@
# {
#   hello = {
#     world = "Hello World!";
#   };
# }
{
  hello.world = "Hello World!";
}

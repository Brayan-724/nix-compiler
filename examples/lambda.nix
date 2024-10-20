# Lambdas
# 
# Test:
#   - Lambda creation and execution
#   - Single param
#   - "Multiparam"
#   - Pattern param
#   - Pattern bind param
#   - Recursion
#   - Clousures
# 
# The output must be:
#@@@
# {
#   single = "Hello World!";
#   multiple = "Hello World!";
#   pattern = "Hello World!";
#   pattern-bind = "Hello World!";
#   recursion = "Hello World!";
#   clousure = "Hello World!";
# }
let
  # single = name: "Hello ${name}!";
  # multiple = greeting: name: "${greeting} ${name}!";
  pattern = input@{ greeting ? "Hello", name, ... }: "${greeting} ${name}${input.exclamation}";
in {
  # single = single "World";
  # multiple = multiple "Hello" "World";
  pattern = pattern { name = "World"; exclamation = "!"; };
}

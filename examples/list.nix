# Test list specific builtins
#@@@
# true

# elem
assert !(builtins.elem 0 []);
assert builtins.elem 1 [1 2 3];
assert builtins.elem 2 [1 2 3];
assert builtins.elem 3 [1 2 3];
assert !(builtins.elem 4 [1 2 3]); 

# If everything is ok, then return true
true

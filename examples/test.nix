builtins.mapAttrs (k: v: builtins.trace ("map " + k) v) rec {
  b = builtins.trace "b" {
    inherit a;
  };
  d = builtins.trace "d" {
    inherit a b c;
  };
  a = builtins.trace "a" "a";
  c = builtins.trace "c" b;
}

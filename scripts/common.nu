export def "duration colored" [] : duration -> string {
  if $in < 20us {
    $"(ansi gd)($in)"
  } else if $in < 50us {
    $"(ansi g)($in)"
  } else if $in < 500us {
    $"(ansi yb)($in)"
  } else if $in < 1ms {
    $"(ansi lrb)($in)"
  } else {
    $"(ansi bg_r)($in)"
  }
}

export def "step start" [-n, name: string] : nothing -> datetime {
  print -n $"(ansi yb)(char prompt) ($name)(if $n { "\n" })"

  date now
}

export def "step rename" [-n, name: string] : nothing -> datetime {
  if not $n {
    # Go to the start of line
    print -n $"(ansi erase_entire_line)(ansi csi)1G"
  }

  print -n $"(ansi yb)(char prompt) ($name)(if $n {"\n"})"

  date now
}

export def "step end" [-n, name: string, start?: datetime] {
  if not $n {
    # Go to the start of line
    print -n $"(ansi erase_entire_line)(ansi csi)1G"
  }

  if $start != null {
    print $"(ansi gb)(char prompt) ($name) in ((date now) - $start)"
  } else {
    print $"(ansi gb)(char prompt) ($name)"
  }
}

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

export def "compare colored" [actual: cell-path, before: cell-path] : record -> string {
  let dur = ($in | get $actual)
  let dur_before = ($in | get $before)
  let percent = if $dur_before != null { ($dur) - $dur_before } else { 0 }
  let percent = ($percent / $dur * 10000 | into int) / 100

  let percent = match 0 {
    _ if $percent > 0 => $" \((ansi lrb)($percent)%(ansi reset)\)"
    _ if $percent < 0 => $" \((ansi g  )($percent)%(ansi reset)\)"
    _ => " (0%)"
  }

  $"($dur | duration colored)(ansi reset)($percent)"
}

export def "step start" [-n, name: string] : nothing -> datetime {
  print -n $"(ansi yb)(char prompt) ($name)(ansi reset)(if $n { "\n" })"

  date now
}

export def "step rename" [-n, name: string] : nothing -> datetime {
  if not $n {
    # Go to the start of line
    print -n $"(ansi erase_entire_line)(ansi csi)1G"
  }

  print -n $"(ansi yb)(char prompt) ($name)(ansi reset)(if $n {"\n"})"

  date now
}

export def "step end" [-n, name: string, start?: datetime] {
  if not $n {
    # Go to the start of line
    print -n $"(ansi erase_entire_line)(ansi csi)1G"
  }

  if $start != null {
    print $"(ansi gb)(char prompt) ($name) in ((date now) - $start)(ansi reset)"
  } else {
    print $"(ansi gb)(char prompt) ($name)(ansi reset)"
  }
}

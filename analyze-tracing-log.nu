def "colored" [] : duration -> string {
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

def "step start" [name: string] : nothing -> datetime {
  print -n $"(ansi yb)(char prompt) ($name)"

  date now
}

def "step rename" [name: string] : nothing -> datetime {
  # Go to the start of line
  print -n $"(ansi erase_entire_line)(ansi csi)1G"

  print -n $"(ansi yb)(char prompt) ($name)"

  date now
}

def "step end" [name: string, start?: datetime] {
  # Go to the start of line
  print -n $"(ansi erase_entire_line)(ansi csi)1G"

  if $start != null {
    print $"(ansi gb)(char prompt) ($name) in ((date now) - $start)"
  } else {
    print $"(ansi gb)(char prompt) ($name)"
  }
}

def "main" [] {
  let tracing_file = open ./tracing.log

  let start = step start "parsing logs"

  let parsed = $tracing_file 
  | lines
  | parse '{path} exit in Ok({time})'

  let entries = $parsed | length
  step rename $"collecting ($entries) entries"

  let parsed = $parsed
  | compact
  | update path {|it| 
    $it.path 
    | split row ":" 
    # Last is always an empty string
    | drop
    | last
    | str trim
  }
  | group-by --to-table path

  step end $"parsed ($entries) entries" $start

  $parsed
  | upsert results {|it|
    let start = step start $it.path

    let items = $it.items
      | par-each {|it| 
        $it.time 
        | str replace -r '\.\d+' "" 
        | str replace -r '(\d)s' "$1 sec" 
        | str replace -r ' ' "" 
        | into duration 
        | into int
      }

    let out = [
      [total min max mean median stddev];
      [
        ($items | length)
        ($items | math min | into duration | colored)
        ($items | math max | into duration | colored)
        ($items | math sum | into duration)
        ($items | math median | into int | into duration | colored)
        ($items | into float | math stddev | into int | into duration | colored)
      ]
    ]
    | upsert mean {|it| ($it.mean / ($items | length)) | colored}

    step end $it.path $start

    $out
  }
  | reject items
  | flatten results --all
  | table -e -i false
}

use ./common.nu *

def "main" [path?: path] {
  main process $path
  | update min {duration colored}
  | update max {duration colored}
  | update mean {duration colored}
  | update median {duration colored}
  | update stddev {duration colored}
  | update impact {duration colored}
  | table -e -i false
}

def "main process" [path?: path = ./tracing.log] {
  let result = try {
    open $path
    | from nuon
  } catch {
    main get-raw $path
  }

  $result
  | update min {duration colored}
  | update max {duration colored}
  | update mean {duration colored}
  | update median {duration colored}
  | update stddev {duration colored}
  | update impact {duration colored}
  | table -e -i false
  | print

  $result
  | to nuon
  | save -f $path

  $result
}

def "main compare" [a: path, b?: path = ./tracing.log] {
  print $"Before: ($a)"
  let a = main process $a

  print $"Actual: ($b)"
  let b = main process $b

  def "compare results" [actual: cell-path, before: cell-path] : table -> table {
    $in
    | update $actual {|it| $it | compare colored $actual $before}
    | reject $before
  }

  print "====== Results:"
  $b
  # min
  | merge ($a.min | wrap before_min)
  | compare results $.min $.before_min
  # max
  | merge ($a.max | wrap before_max)
  | compare results $.max $.before_max
  # mean
  | merge ($a.mean | wrap before_mean)
  | compare results $.mean $.before_mean
  # median
  | merge ($a.median | wrap before_median)
  | compare results $.median $.before_median
  # # stddev
  # | merge ($a.stddev | wrap before_stddev)
  # | compare results $.stddev $.before_stddev
  | reject stddev
  # impact
  | merge ($a.impact | wrap before_impact)
  | compare results $.impact $.before_impact
  # 
  | reject total
  | table -e -i false
}

def "main get-raw" [path: path] {
  let tracing_file = open $path

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
      [total min max mean median stddev impact];
      [
        ($items | length)
        ($items | math min | into duration)
        ($items | math max | into duration)
        ($items | math sum | into duration)
        ($items | math median | into int | into duration)
        ($items | into float | math stddev | into int | into duration)
        ($items | math sum | into duration)
      ]
    ]
    | upsert mean {|it| ($it.mean / ($items | length))}

    step end $it.path $start

    $out
  }
  | reject items
  | flatten results --all
  | rename name
  | sort-by name
}

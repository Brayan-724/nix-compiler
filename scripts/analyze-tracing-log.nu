use ./common.nu *

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
      [total min max mean median stddev impact];
      [
        ($items | length)
        ($items | math min | into duration | duration colored)
        ($items | math max | into duration | duration colored)
        ($items | math sum | into duration)
        ($items | math median | into int | into duration | duration colored)
        ($items | into float | math stddev | into int | into duration | duration colored)
        ($items | math sum | into duration | duration colored)
      ]
    ]
    | upsert mean {|it| ($it.mean / ($items | length)) | duration colored}

    step end $it.path $start

    $out
  }
  | reject items
  | flatten results --all
  | rename name
  | sort-by name
  | table -e -i false
}

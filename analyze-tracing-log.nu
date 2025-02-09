def "main" [] {
  let tracing_file = open ./tracing.log

  $tracing_file 
  | lines
  | skip until {|it| $it =~ "Running"}
  | skip 1
  | parse '{path} exit in Ok({time})'
  | update path {|it| 
    $it.path 
    | split row ":" 
    # Last is always an empty string
    | drop
    | last
    | str trim
  }
  | group-by --to-table path
  | upsert results {|it|
    let items = $it.items
      | par-each {|it| 
        $it.time 
        | str replace -r '\.\d+' "" 
        | str replace -r '(\d)s' "$1 sec" 
        | str replace -r ' ' "" 
        | into duration 
        | into int
      }
      | inspect

    {
      total: ($items | length),
      min: ($items | math min | into duration),
      max: ($items | math max | into duration),
      mean: ($items | math sum | into duration),
      median: ($items | math median | into int | into duration),
      stddev: ($items | into float | math stddev | into int | into duration),
    }
    | upsert mean {|it| $it.mean / ($items | length)}
  }
  | reject items
  | table -e -i false
}

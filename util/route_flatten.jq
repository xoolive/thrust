# This script merges the output of `field15_resolve` into a simplified sequence of points
# Remove altitude and speed but keep full start/end objects
map({start, end})

# Build sequence: all start objects + last end object
| (map(.start) + [last | .end])

# Remove consecutive duplicates (same object)
| reduce .[] as $p ( [];
    if (. | length) > 0 and .[-1] == $p
    then .
    else . + [$p]
    end
)

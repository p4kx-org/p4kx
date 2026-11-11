# Field statistics are counted per path in one parallel pass over the instances

Decided for issue #267.

A Query needs the right field, and DataForge has tens of thousands of field paths, many of them
only below one polymorphic pointer. `FieldStatistics` in `p4kx-dataforge` answers "which paths
exist, how often, of which kind, between which numbers" for all Records or for a selection.

The pass reads original instance bytes through cached struct layouts and builds no Record tree.
A trie node is a field name or the actual struct name below a strong pointer; array positions
are not path segments. Field-position and actual-type caches reuse the association between a
visited declaration and its trie node. Scalar observations enter a per-Record pending list and
are committed after the walk. Text fingerprinting still reads the complete selected text.

Every node accumulates value, Record, empty and per-kind counts, numeric extrema and a distinct
summary. The summary retains at most32 distinct64-bit fingerprints. A further distinct value sets
the capped flag and releases its fingerprint list. A capped input remains capped after merging.
Counters add, fingerprints union and extrema merge; output is sorted by path. The same valid
Record selection retains the existing thread-independent result contract. Fingerprints can
collide and are not original value identity. Numeric extrema use the existing Integer(i128),
Single(f32) and Double(f64) representations and their existing comparison rules.

Empty values remain occurrences: null pointers/References, empty text and a null GUID add to
`values` and `empty`, without distinct or numeric extrema. Zero and false remain values.
Negative zero is normalized to positive zero for number comparison/fingerprinting; NaN has no
numeric extremum. An empty array has no element and contributes no element occurrence.

## Iterative source traversal and retained state

The walker uses heap-backed structure/array continuations, without a fixed owned-depth or mixed
Record-value/work cutoff. Each actual owned instance is followed once per Record. Active inline
declaration types are tracked within their actual owner, so cyclic inline Class declarations are
rejected separately from valid deep or repeated acyclic structure.

An invalid requested Record position, root instance or root layout increments unreadable Records.
A rejected array range, missing instance, unreadable nested layout, repeated owner, inline cycle
or unreadable scalar increments the applicable unreadable-value count. Where a leaf occurrence
is retained its kind/count remains observable; unreadable scalars add no distinct or extremum.
Descendants behind a rejected source edge are not invented or visited. These raw-statistics
counters are not a second canonical ValueError inventory.

The fold minimum is the ceiling of selected Record count divided by rayon threads, at least1,
which bounds nonempty fold parts by that thread count. Each part has a trie, per-Record pending
observations, visited instances, path/type caches and iterative traversal state. Reduction also
needs live input tries and an index-remapping buffer; final output materializes complete path
strings. Thread count alone is not a whole-process memory bound.

The pending list grows with the actual admitted Record and is drained on commit. Its capacity
is released after commit when above `KEPT_PENDING` (2^16); this is retained-scratch policy,
not an admission limit. Other caches and capacities also contribute. No uniform64MiB-per-part
bound follows from the removed value quota or from the size of a Value/Pending element.

## Separate existing policies and merging

- `MAX_FIELD_PATHS` is the existing trie-node policy of1,048,576 nodes INCLUDING its empty root.
  `Trie::child` refuses a further node and merge uses that same insertion path. It therefore
  permits at most1,048,575 non-root path nodes, not1,048,576 independent returned fields.
  It is an implementation policy, not a DataForge format maximum.
- Exceeding that policy while discovering or merging paths returns `TooManyPaths`, not a
  successful prefix. Paths discovered before an unreadable edge still consume trie nodes.
- `SharedLayouts` provides one slot per original struct and caches retained layout fields under
  `CACHE_FIELDS` (2^20). A layout outside that cache policy is read again as needed; source
  occurrences are not truncated by cache admission. Active owned layouts and cache metadata
  remain additional state.
- `MAX_DISTINCT_VALUES` remains32. It bounds the distinct summary, not traversal or occurrence
  counts. Strong/weak pointers and inline structs add no distinct value; Reference observations
  fingerprint the stored non-null GUID without following the target Record.
- A stop is checked before each Record. `of_records_while` returns `None` after a stop, not a
  successful partial statistics object. Path errors likewise return an error.
- `Part::merge` combines the counts and tries and drops part-local scratch. Source traversal,
  path insertion and the separate node policy do not depend on a new depth/value threshold.

The earlier recorded LIVE run (117,709 Records,87,514 paths,32 threads) observed about53MB above
the opened DataForge, compared with about331MB for the older per-task caches. This is historical
evidence for that run, not a current measurement, future Build guarantee or bound for the
iterative implementation. Actual peak storage depends on paths, spelling lengths, admitted
Record observations, live parts, traversal/visited state, caches and merge/output overlap.

The same walk serves completion: `get` and `children` expose observed kinds and child paths.
Schema completion remains separate and can describe declarations without an observed value.
See [canonical owned values](../canonical-owned-values.md) for canonical ownership and consumer
boundaries; no parser, RSS, unlimited-source or constant-memory guarantee is implied.
## Considered Options

- Build the Record tree of every Record and walk it: the tree allocates for every value, and
  these counters need none.
- Keep every distinct value with its count (top values): memory grows with the data, and the
  question this answers is which field to use, not which values it has; 32 fingerprints say
  whether a field is constant, a flag or free, and the extremes say its range.
- Count by the declared struct instead of the actual struct of a strong pointer: paths of
  polymorphic arrays would merge fields of different structs, which is what the statistics are
  there to tell apart.
- Cap the paths silently and report a count: the cut paths would depend on the order the Records
  are read in, and the result would differ between runs.

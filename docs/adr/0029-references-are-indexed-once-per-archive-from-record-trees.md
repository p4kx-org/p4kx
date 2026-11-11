# References are indexed once per Archive from the Record trees

"Which Records refer to this Record?" cannot be answered from the stored data without reading
every Record: a Reference is stored in the instance of the Record that holds it, and nothing
points back. StarBreaker answers it per call by parsing the DataForge again and scanning only the
main Records, so it misses References held by non-main Records. The index must cover every
Record, keep the field and the array elements of each Reference, keep References to a GUID no
Record has, and be built once per open Archive: the DataForge of LIVE holds millions of
References.

`ReferenceIndex` in `p4kx-dataforge` reads the canonical tree of every Record
(`TreeBuilder::build`, one builder per rayon task) and walks it, so the field path and the array
indices of a Reference are exactly those of the Record preview and the Query, and a Reference
held by a struct nested in an array is found like any other. A Reference field, a weak pointer to
another Record and a dangling Reference are References; a Reference or weak pointer into the
Record that holds it is not, because "the Record refers to itself" says nothing about its place
among the other Records. A weak pointer to a nested instance of another Record is not indexed
either: the Record tree reports it as outside the Record (`ValueError::OutsideRecord`) and does
not know which Record owns the instance, so only a weak pointer to the instance a Record is
defined by links the two Records.

The index stores References in columns and shared buffers. Each target is the physical Record
selected by the decoder, including when two Records share a GUID. Occurrence positions and buffer
offsets use the platform's addressable width. On a 64-bit target the wider columns take 12
additional bytes per indexed occurrence. Source and target Record indices and original array
indices stay 32-bit. A shape is `Components[].fireActions[].ammo`;
the text with indices, `Components[3].fireActions[0].ammo`, is built on demand. Each parallel task
collects its Records in order with a local interner; the tasks run in waves of a few per thread
and the chunks of a wave are appended in Record order and freed as they are moved, so the
outgoing References of a Record are one run, and incoming ones are grouped by target with one
counting pass. Canonical owned traversal uses the actual source values without a mixed
value/work cutoff; [canonical owned values](../canonical-owned-values.md) describes its costs.
The ordinary build indexes every readable Reference. An explicit occurrence budget retains the
first occurrences in Record order and signals truncation at the first omitted Reference.
Unreadable Record trees are counted separately. The build polls cancellation between Records. All of it runs outside
any `OnceLock` initialiser, because the build uses rayon.

The window builds the index lazily, the first time the "Referenced by" tab is shown or a Query
with `referencing` runs, and keeps it in the shared cache of the Archive next to the completion
schema: the build is serialised by a mutex, a consumer that waits for it polls its own stop
flag, the result is stored afterwards, and a change of the Archive on disk drops it with the
DataForge. The "Referenced by" tab has one worker thread for all its lists, which serves the
newest request after the build, and dropping the Archive stops the build; a Query run shows
"Indexing References" while it waits and stops with its Cancel. The CLI builds the index for the
one command. `Query::compile` builds one after the type and field names are checked and only
for a `referencing` Record that exists; `Query::compile_with` without an index is an error for a
`referencing` Query.

The file graph groups Records by record path (ignoring case, separators and `.xml`, as the
Record search does) and has an edge from file A to file B, A different from B, when a Record of A
refers to a Record of B. A chain of References through Records of one file ends at the files it
reaches, so the edges of the Records of a file already contain what a walk through the file would
add. Unresolved References give no edge. On LIVE this gives about 475,000 edges for 62,700 files.

## Considered Options

- Scan the instance bytes by struct layout instead of Record trees: fewer allocations per Record,
  but it repeats the ownership and weak-pointer logic of the tree builder and would give
  different field paths in corner cases.
- A path string per Reference: simpler, but 30 to 60 bytes more for each of millions of
  References.
- Build the index in the DataForge on first use behind a `OnceLock`: a rayon pool call inside the
  initialiser deadlocks when another thread of the pool waits for the same lock.

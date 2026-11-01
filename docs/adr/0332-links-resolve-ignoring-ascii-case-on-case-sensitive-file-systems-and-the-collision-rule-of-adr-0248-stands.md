# Links resolve ignoring ASCII case on case-sensitive file systems, and the collision rule of ADR-0248 stands

Decided for issue #1018 (part of #827, Linux). Extends ADR-0248.

## Decision

- **Colliding Entries.** The rule of ADR-0248 holds unchanged on every file system: Entries whose
  paths differ only in case collide, the first one in the order of the run is written and each
  later one is reported as `ExtractionError::CaseCollision`. A case-sensitive file system never
  receives two spellings of one path. The rule is the same on Windows and Linux, so no test
  depends on what the file system does with case.
- **Files an Extraction finds already in the target** are matched by the exact name the Entry
  has; what the file system makes of a name with other case is its own business and is not
  emulated.
- **Names are written and opened as they are.** An Entry is written under the case of its Archive
  path, and a folder listing opens the file under the name the listing gave. No path is folded,
  lower-cased or normalised before it is opened; the fold of ADR-0248 only compares.
- **A link in a game file** (a companion file or material library of a model, an external image of
  a GFx movie) names a file ignoring ASCII case, as the Archive lookup (`PathIndex`) does. When
  the Entry is read from disk and the file with exactly that name is missing, a non-Windows build
  lists the folder and uses the one file whose name equals the link's ignoring ASCII case, so a
  folder extracted on Linux converts to the same result as on Windows. Two or more candidates are
  ambiguous and count as no file. Windows opens the name as written and never lists. The check
  runs only after the exact name failed, so the common path allocates nothing new.
- **`FileSource`** opens the Archive read-only with a Windows share mode of read and write and
  plain `open` elsewhere; `read_exact_at` returns `UnexpectedEof` for a short read or a read at
  or past the end on both platforms. Both are covered by tests that run on every platform.
- **The Linux counterparts of the `cfg(windows)` link tests** in `folder_links.rs` and
  `folder_conversion.rs` already exist as the `cfg(unix)` branch of the `link` helper (a symbolic
  link instead of a junction); the test of a swapped folder now fails on Unix when a link cannot
  be created instead of passing without checking anything.

## Consequences

- On Linux a model whose chunk or material link differs in case from the extracted file converts
  as it does on Windows.
- Texture parts (`.dds.1`, `.dds.1a`) are still found on disk by the name derived from the header
  file, as written. Parts are probed until one is missing, so a folder listing per texture would
  cost more than the case difference between a header and its parts is likely to be worth; a
  follow-up can measure it on a real Archive.

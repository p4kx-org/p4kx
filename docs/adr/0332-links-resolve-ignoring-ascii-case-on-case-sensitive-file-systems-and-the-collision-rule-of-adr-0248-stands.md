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
  ambiguous and count as no file. Windows opens the name as written and never lists.
- **The fallback runs only after exact-name absence (`NotFound`).** Exact hits do not allocate or
  read a fallback listing. Other metadata or open errors, including `PermissionDenied` and
  `NotADirectory`, retain their original I/O cause and never trigger a different-case fallback. The
  first miss in a folder lists it once into a `CaseIndex` (lower-case name to candidates) that the
  resolver keeps for one conversion run, so further misses in that folder cost a lookup. A missing folder
  counts as no file. Other listing failures are cached and returned as I/O errors, including a
  failure after some directory entries have been read; a partial listing is never used as complete.
  Replayed OS errors retain their raw code and kind; other I/O errors retain their original cause.
- **Disk-image errors retain their role.** A missing or ambiguous external image is missing; an
  absolute path, lexical escape or canonical escape is outside the allowed root. Filesystem errors
  while resolving or canonicalising an image or its root remain read errors.
- **Disk texture parts share their run.** `DiskTextures` owns one `CaseIndex` and one cache of
  requested paths bound to exact paths, unique actual paths, absence or an I/O failure. The cache
  uses absolute requested paths and preserves the actual spelling. Header lookup, numbered colour
  parts, alpha headers and numbered alpha parts use that context. An established binding is not
  replaced when a differently named candidate appears later; exact names take precedence when a
  path is first bound. Successful, missing and failed outcomes remain consistent across metadata,
  reads and output-input checks.
- **The disk context has operation scope.** The SDK can share `DiskTextures` across its conversions;
  `Texture::from_file` wraps a fresh one-texture run. One UI asset extraction or frame preparation
  creates one context outside its parallel jobs and passes it to external header and part lookup.
  A subsequent operation creates a fresh cache. Existing model companion contexts keep their
  model-conversion lifetime. `FolderSource` keeps its guarded scan and `Parts::Source` access.
- **Sharing is confined to the disk context.** A single `Arc<Mutex<DiskFileIndex>>` retains the
  owned run across cloned alpha Parts and parallel textures without borrowing a mutable cache
  through a texture. It avoids eager probing of unused part numbers and premature alpha parsing.
  The lock covers resolution and short metadata/open calls, never streaming reads, decoding or
  output writes. Complete outcomes are inserted after lookup; poison recovery retains the valid
  cache with the same `PoisonError::into_inner` policy as the existing extraction caches. There
  is no per-part lock, process-global cache, new matching algorithm or native API dependency.
- **Alpha parsing and errors remain lazy.** Construction captures the main raw header without
  requiring a known byte layout or computing part count. Established alpha presence is reported
  by `has_alpha_tail`; a failed presence lookup is retained separately and `alpha_tail` reports
  it when used. An alpha-consuming decode never silently drops that failure. Malformed contents
  of a known actual alpha header still permit colour-only merging; that header remains protected.
  Input inventory binds the numbered inputs of usable layouts before a write. An unresolved alpha
  lookup or an I/O failure opening or reading its header prevents file-output preflight and
  returns the original typed read failure before an output writer is opened. A requested spelling
  never substitutes for an unproved input path. Colour-only `merge_into` with a caller-supplied
  writer remains available. The change adds no general read prohibition for excluded model or
  source inputs.
- **The fallback does not backtrack.** A part that exists as written is kept; if `Textures` and
  `textures` both exist, a link into `textures` is not found once `Textures` exists and lacks the
  file.
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
- On Linux colour parts, alpha headers and alpha parts resolve uniquely ignoring ASCII case;
  metadata, reading and overwrite refusal use the retained physical paths.
- An explicitly supplied SDK or CLI header path still has to exist as written. Linked UI header
  resolution uses its context's unique fallback and retains canonical confinement to its image root.

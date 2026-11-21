# A DataForge file on disk is read by its content, without an Archive

Decided for issue #862. Builds on ADR-0136 (versions 6 and 8 only) and the shared DataForge
input policy of ADR-0283.

## Decision

- **A `Game2.dcb` kept from an earlier patch is read like the Entry of an Archive.** Every
  `p4kx dcb` command (`info`, `export`, `query`, `fields`, `refs`, `tags`), the DataForge tools of
  the MCP server and the window accept the file where they accept an Archive.
- **The file is recognised by its content, not by its name.** The header decides
  (`looks_like_dataforge`): a reserved word of zero, a version word from 1 to 65,535, and, unless
  the version is 6 or 8, table counts whose rows fit the file. Any such version passes, so that
  version 5 and older end as the typed `DataForgeError::UnsupportedVersion` of ADR-0136 and exit
  code 5, not as an unknown file; a version 6 or 8 file may claim more than it holds, so that a
  truncated DataForge is reported as truncated. A zero-filled file, an executable, a gzip file or
  an Archive is no DataForge. A file shorter than the header (about 120 bytes, so every file
  under 8 bytes too) is none either. A file that is neither an Archive nor a DataForge is an
  Archive error, exit code 3; an I/O error while looking at the file is a read error, exit code 3.
- **A file replaced while it is read** is reported as changed, like an Archive.
- **File and Entry use the same input policy.** Both support DataForge versions 6 and 8.
  [ADR-0283](0283-a-dataforge-is-read-through-one-source-type-and-a-file-has-no-archive-in-the-window.md)
  defines platform capacity, optional caller admission and cancellable input buffering. The file
  is opened read-only.
- **Texts and tags.** A DataForge holds no localisation texts. `dcb export --lang` on a file needs
  `--archive <Data.p4k>` for them and exits with code 2 without it; an Archive as input supplies
  its own and refuses `--archive`. Tag paths resolve from the DataForge alone.
- **Without Channel and build** the default export folder is
  `SC-EXPORT-UnknownChannel-UnknownBuild-<timestamp>`, and `--combined` is named after the file.
- **MCP.** The `archive` argument of the five DataForge tools takes the absolute path of a `.p4k`
  or a `.dcb` file, on a local drive like before; every other tool still takes `.p4k` only. The
  extension stays required: a tool call comes from a model, and recognition by content alone
  would let it probe any file of the machine. The read DataForge and the selection of the last
  Query are kept per file version.
- **Window.** The Archive menu offers **Open DataForge file…**; dropping a file or opening one
  through **Open Archive…** works as well. The Records, Query and Tags views use the one
  DataForge cache they use for an Archive. The tabs that read Entries are hidden and the
  Basetool catalog is unavailable for a file. How the window holds a file without an Archive is
  decided in ADR-0283.

## Reasons

- Users keep `Game2.dcb` files of old patches, and the Archive they came from is often gone.
- Recognition by content serves a renamed file; the version word, not the extension, separates a
  DataForge from an Archive.
- A stand-in Archive in the window reuses every consumer of the DataForge cache instead of a
  second code path through them.

# Converting disk textures

`Texture::from_file` opens the explicitly named DDS header and creates one conversion run. The
header path has to exist as written. Numbered colour parts, an alpha header and numbered alpha
parts use exact names first. On Linux and other non-Windows systems, a missing exact name can
resolve to a unique filename differing only in ASCII case. Ambiguous siblings remain missing;
permission failures and other filesystem errors retain their original cause.

The SDK's `DiskTextures` handle shares folder listings and selected physical paths across several
textures in one run. Create a new handle for a new run; cloning it shares the existing run.

```rust
use std::path::Path;
use p4kx_extraction::DiskTextures;

let files = DiskTextures::new();
let first = files.open(Path::new("textures/first.dds"))?;
let second = files.open(Path::new("textures/second.dds"))?;
first.merge_to_file(Path::new("converted/first.dds"))?;
second.merge_to_file(Path::new("converted/second.dds"))?;
```

The run retains each path's first selection, absence or lookup failure. Starting another run
refreshes that view. An exact file that has not been looked up yet remains usable even when its
folder's fallback listing is already cached. The retained paths coordinate size checks, actual
reads and refusal to overwrite inputs; they do not provide a filesystem snapshot against external
replacement of the files themselves.

Alpha headers are parsed only where used. A malformed alpha header does not prevent merging
the colour DDS. An unreadable alpha lookup is retained and fails an operation that uses alpha;
`has_alpha_tail` reports established presence, while `alpha_tail` also reports lookup failure.
An unresolved input lookup, an input open failure or a header read failure also prevents writing
a file, because overwrite protection cannot establish all relevant input paths. The original
I/O error is returned. Successfully read but malformed header bytes remain a content error:
the known alpha header stays protected while colour-only merging remains available. Merging
colour bytes into a supplied writer does not read alpha and remains available even when its
header cannot be opened or read.
Raw source header metadata remains available even when the byte layout has no supported decoder.

UI asset extraction and frame preparation share one disk context across their parallel external
image jobs. Each later operation creates a fresh context. Folder conversion keeps its existing
guarded source scan. Windows retains its native exact-name behaviour without fallback listings.

The detailed ownership and failure rules are defined in
[ADR-0332](adr/0332-links-resolve-ignoring-ascii-case-on-case-sensitive-file-systems-and-the-collision-rule-of-adr-0248-stands.md).

# Installation declarations

## Reader policy

An ordinary v2 open refuses status bit 0 with ArchiveError::IncompleteInstallation, retaining
the complete raw status byte in the error. The action is to let the launcher finish installing.
The tool does not repair or write the Installation. Existing v1 directory recovery is not a bypass
for a v2 installation declaration.

A successfully opened v2 Archive exposes InstallationStatus::DeclaredComplete { flags }.
Only bit 0 is interpreted. Other bits are retained unchanged and carry no invented meaning.
The declaration is separate from payload integrity and selection/export completeness.
V1 and plain ZIP Archives expose InstallationStatus::Unavailable, not confirmed completeness.

## Installed-byte counters

The v2 install block begins at the trailer's end-of-payload offset. It contains one little-endian
u64 counter per Entry followed by the declared free-list records. Counts, multiplication, range
ends and the available gap before the directory are checked before allocation. Each counter is
retained as Entry::installed_bytes() and cannot exceed that Entry's stored size. The counter
is a source fact, not a replacement for CRC or SHA-256 verification. Zero and partial counters
remain distinguishable from an unavailable counter.

A declared incomplete Archive cannot reach normal CLI/GUI/MCP extraction/export publication:
all use the same refusing Archive open. No independent frontend recovery flag is added.

## Verification

Synthetic public-API tests cover valid payloads with complete/incomplete declarations, unknown
bits, unavailable v1 metadata, zero Entries, recovery refusal, nested metadata preservation,
zero/partial/full counters, excessive progress and truncated/overflowing install ranges.
The synthetic v2 builder writes actual install counters, independently of the reader.

The supported layout facts are traced to the
[pinned trailer description](https://github.com/19h/svarog/blob/2e46eff4f7d475b8826ccfdc01c4a95db764db0d/crates/svarog-p4k/src/zip/eocd_v2.rs)
and [checked reader](https://github.com/19h/svarog/blob/2e46eff4f7d475b8826ccfdc01c4a95db764db0d/crates/svarog-p4k/src/archive.rs).

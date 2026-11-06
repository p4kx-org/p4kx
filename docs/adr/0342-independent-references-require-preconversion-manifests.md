# Independent references require pre-conversion manifests and complete logical coverage

Decided for issue #1104, extending ADR-0234 and integrating ADR-0341.

## Problem

An empty configured folder provides no independent evidence. File counts cannot distinguish
overwritten same-stem XML, repeated GUIDs, omitted requested sources or a partially successful
producer. A version string and process exit code do not identify the actual source/build or prove
that all selected outputs were produced.

## Decision

Use version 1 of the root `.p4kx-reference.json` contract, with generated JSON Schemas for the
manifest, pre-conversion plan and derived error report. A configured root requires that contract;
an unset optional reference variable skips explicitly.

A manual `begin` helper measures the source Archive/Build, canonical Entry inventory, preferred
DCB where applicable and producer binary. It resolves the expected logical identities against
that source and freezes selection plus source-to-artifact mappings before any payload exists.
Explicit source copies may precede conversion only when their bytes match the selected Entry.

The foreign producer is run separately. Manual `finalize` requires an explicit owner completion
receipt, remeasures source/binary identity, checks the frozen mapping and complete output inventory,
derives producer-error findings from captured stdout/stderr, and fingerprints all artifacts.
The helpers never download or execute foreign programs. Failures leave an unfinalized manifest.

The producer's version/revision relationship and success receipt remain owner claims. Binary
pre/post hashes identify the local executable; they are not build attestations or proof of foreign
execution. Source and payload SHA-256 values are independently recomputed locally and distinguished
from claims. Error-report capability means a classifier for a pinned source contract, not attested
binary authenticity.

Known log contracts are unmodified unp4k/unforge b492ab14 and StarBreaker 450fa284/7d3f01e7.
Caught failures can appear on stdout/stderr despite exit zero. Healthy StarBreaker progress on
stderr is valid. A typed report is recomputed from both captured streams, so fabricating an empty
report cannot hide failures. Unknown revisions, local patch scopes, unexpected lines/encoding or
inconsistent completion markers are `review_required` and cannot yield a complete pass.

Coverage is by unique Entry identity or canonical GUID plus DCB identity. Every selected identity
must have its declared output locations and one reconciled comparison result. Exact root-manifest
exclusion is the only enumeration exception; source copies and logs have explicit roles.
Duplicate sources, physical/path aliases and missing/extra artifacts fail. XML source extension
comes only from the mapping; CDF/CHRPARAMS output collisions are rejected before conversion.

A Record can have several artifacts, and several Records can use distinct verified JSON locations
in one file. Standalone/declared embedded coverage is reported separately. Nested comparison visits
can repeat and cannot inflate unique logical coverage. Embedded source headers that the producer
does not expose are not fabricated.

`all` matches the source universe; explicit filtered, sampled and main-only scopes remain bounded
subsets. Main-only disposition remains an explicitly unverified producer claim because the
foreign rule uses raw file-name offsets that the public Record API does not expose; locally
verified subset membership never attests that disposition. A declared empty selection is distinct from a missing manifest and reports no independent
comparisons, never a successful oracle. Unselected Records are not missing selected artifacts.

Manifests, identities/mappings, paths and artifact/structured-input allocations have documented
bounds. Relative path validation, canonical containment and link/alias checks precede reads.
Comparisons validate actual consumed structured bytes or stream fingerprints, retain source-change
checks, and never resolve foreign Record links as host filesystem paths.

## Sources and verification

- [unp4k/unforge entrypoints at b492ab14](https://github.com/dolkensp/unp4k/tree/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src)
- [StarBreaker release extraction](https://github.com/diogotr7/StarBreaker/blob/450fa284d4b1cbed6832675297dfb02579c0abd3/cli/src/dcb.rs)
- [StarBreaker current-source extraction](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/cli/src/dcb.rs)

Synthetic fixtures exercise real preparation/finalization and every adapter, known log failures,
unknown reporting capabilities, source/producer mutations, inventory collisions/omissions, empty
and bounded selections, standalone/embedded Record mappings, path escapes, aliases and bounds.
No game payload, real manifest or foreign binary enters fixtures or hosted CI. Real bounded runs
remain local and publish aggregate counts only.

[The reference guide](../differential-verification.md) defines commands, migration, limits and
producer-version distinctions. Published scunpacked data remains a separate adapter: unavailable
foreign provenance is recorded honestly rather than fabricating a controlled-run manifest.

# Media providers share a bounded writer and generation commit

The media export contract is a service in `p4kx-media`: CLI → Media → Extraction. Format readers
remain independent of Media, CLI and Blender, and Extraction does not depend on Assembly.
Providers describe their version, availability and precise projected field groups, then project
one retained physical source at a time through a common writer. The permanent initial Archive
provider exposes supported public metadata; declaring the raw-layout group unsupported does not
claim that unknown root bytes were preserved. Later domain providers extend the registry.

The envelope is `p4kx-media` version `1.0`, independent of catalog schema versions. Physical
identities contain the root file SHA-256, ordered container Entry indices and the leaf Entry
index. Display paths are reader-exposed paths; original path bytes are included only where the
public Archive API provides them. Provider subresource namespaces and original ordinals retain
duplicate source identifiers. Integrity algorithms, byte scopes and verification states are typed.
A computed digest is distinct from comparison with a source-declared checksum.

A common reference classifier accepts actual domain-reader candidates without another lookup or
read. It retains the exact original identifier, namespaced integer width and lossless decimal
64-bit values. Resolved, missing, ambiguous, not-selected and unavailable states carry proven
target identities and distinct scope/availability reasons. Candidate count and metadata bounds
produce an explicitly incomplete result with observed lower-bound counts; they cannot imply a
missing or uniquely resolved target. Resolution, typed projection and raw preservation are separate
claims. The initial provider context has no Archive handle or implicit companion-read capability.

Every selected payload is retained by a verified public Extraction of exactly one `&Entry` into
its own temporary destination, without name lookup, globbing or conversion. The actual report,
byte count and SHA-256 are checked before the next source. Raw references preserve decoded Entry
payloads; they do not preserve compression, encryption or unknown root layout. The root digest
streams the complete root file with cancellation and ArchiveWatch checks, and names that exact
byte scope. ArchiveWatch is change detection rather than an immutable filesystem snapshot.

The synchronous pipeline has one active source and a bounded container chain. Limits cover root
hashing, observed Entries, decoded source work, individual payloads, records, lines, typed chunks,
output work and container depth. JSONL and the artifact inventory stream to disk; the root index
does not accumulate an archive-wide list of payloads. Scalar source floats retain finite values
and original IEEE-754 words, with explicit null plus raw bits for non-finite values. A delegating
serializer rejects unwrapped non-finite numbers during the actual bounded write.

A separate streamed source inventory maps every selected retained Entry to its physical identity
and usable raw bytes, independently of domain projection. All providers may skip an unsupported
payload without losing that source mapping or inventing a complete semantic record.

A generation is staged on the destination filesystem. Artifacts are closed, synchronised and
verified before an immutable generation directory is placed. The root `media-index.json` is the
only complete-generation commit point. Partial projections retain an explicitly incomplete
diagnostic generation; errors and cancellation do not replace an earlier complete index. Conflict
policies apply to the complete index, with SHA-256 content comparison for `if-changed`.
An OS file lock on a persistent lock file prevents concurrent writers and is released by kernel
handle lifetime after a crash; the lock pathname is never removed while writers can use it.

File synchronisation and same-volume replacement protect completed writes on supported local
filesystems. Linux directory synchronisation also orders namespace changes. The implementation
does not promise power-loss atomicity on every Windows, network or virtual filesystem. Recovery
uses the still-valid root index and leaves unreferenced generations available for inspection.
Synthetic provider, process, corruption, budget and filesystem-boundary tests exercise these
contracts; opt-in real-Archive tests declare a deterministic bounded selection without implying
full media-provider coverage.

Public capability, provenance and output structs are non-exhaustive; provider-facing constructors
cover the required declarations without exposing publication control. Extensible reason, origin,
format-version and layout enums are non-exhaustive. Fixed projection/diagnostic/reference status
ontologies, the media format discriminator and primitive byte order are closed. Availability,
candidate-inventory completeness and applicability are complementary two-state results. These are
the Google Rust guide's fixed-ontology/two-state exceptions. ExportOptions intentionally supports
functional record update from Default under that guide's option-struct exception.

One output-path module rejects redirected ancestors or leaves before output access. Windows checks
the FILE_ATTRIBUTE_REPARSE_POINT bit as well as symbolic links, including junctions and other
reparse-point types. Output, staging, index, lock and artifact paths share that check. Installation
protection remains the common Guard service's responsibility; source input aliases are not rejected
by these output-only rules. These checks do not claim an adversarial race-free filesystem snapshot.

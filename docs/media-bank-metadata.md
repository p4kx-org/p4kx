# Soundbank metadata in the media export

`p4kx media export ARCHIVE --out DIR --kind audio --filter '*.bnk'` retains each selected
BNK Entry and streams its original physical directory into `audio.jsonl`. The physical source
identity includes the Archive fingerprint, containing Entry chain and leaf Entry index.
Short IDs and display paths may repeat. The source's exact verified bytes are in `source.raw`.

A recognized Wwise BNK or loose WEM path emits one `audio-source-language` row at empty
indices. Its `data.record_type` is `source-language`. `data.payload.language_folder` holds the
original directory name, or `null` for a recognized neutral placement. An unrecognized path
omits this row. The archive directory path is the source of the field. The BNK's numeric BKHD
language ID has its own payload receipt. Embedded WEMs share their owning BNK's physical
source identity. The folder describes physical placement independently of requested language
and fallback selection.

| Namespace | `data.record_type` | Original subresource indices |
| --- | --- | --- |
| `audio-bank` | `bank` | BKHD section ordinal |
| `audio-bank-section` | `bank-section` | Section ordinal |
| `audio-bank-media` | `bank-media` | DIDX section and triplet ordinals |
| `audio-bank-object` | `bank-object` | HIRC section and object ordinals |
| `audio-bank-hirc-action` | `bank-hirc-action` | HIRC section and object ordinals |
| `audio-bank-hirc-action-properties` | `bank-hirc-action-properties` | HIRC section and owning Action object ordinals |
| `audio-bank-hirc-action-property` | `bank-hirc-action-property` | HIRC section, owning Action object, property list and original property ordinals |
| `audio-bank-hirc-action-target-resolution` | `bank-hirc-action-target-resolution` | Local Play or PlayEvent target inventory and foreign lookup availability |
| `audio-bank-hirc-action-target-candidate` | `bank-hirc-action-target-candidate` | One same-ID local HIRC object with its physical type and match flag |
| `audio-bank-foreign-target-resolution` | `bank-foreign-target-resolution` | Directory-local query index |
| `audio-bank-foreign-target-candidate` | `bank-foreign-target-candidate` | Query index and candidate ordinal |
| `audio-bank-foreign-target-blocker` | `bank-foreign-target-blocker` | Query index and blocker ordinal |
| `audio-bank-hirc-event` | `bank-hirc-event` | HIRC section and Event object ordinals |
| `audio-bank-hirc-event-action` | `bank-hirc-event-action` | HIRC section, Event object and action ordinals |
| `audio-bank-hirc-event-action-resolution` | `bank-hirc-event-action-resolution` | Same HIRC section, Event object and action ordinals |
| `audio-bank-hirc-event-action-candidate` | `bank-hirc-event-action-candidate` | HIRC section, Event object, action and candidate ordinals |
| `audio-bank-hirc-event-tail` | `bank-hirc-event-tail` | HIRC section and Event object ordinals |
| `audio-bank-hirc-sound` | `bank-hirc-sound` | HIRC section and Sound object ordinals |
| `audio-bank-hirc-sound-media-resolution` | `bank-hirc-sound-media-resolution` | HIRC section and Sound object ordinals |
| `audio-bank-hirc-sound-media-candidate` | `bank-hirc-sound-media-candidate` | HIRC section, Sound object and candidate ordinals |
| `audio-bank-hirc-sound-loose-wem-resolution` | `bank-hirc-sound-loose-wem-resolution` | Physical loose WEM candidates in the Sound's owning Archive |
| `audio-bank-hirc-sound-loose-wem-candidate` | `bank-hirc-sound-loose-wem-candidate` | Physical Entry ordinal of one canonical loose WEM observation |
| `audio-bank-hirc-sound-rtpc` | `bank-hirc-sound-rtpc` | HIRC section and Sound object ordinals |
| `audio-bank-hirc-sound-rtpc-curve` | `bank-hirc-sound-rtpc-curve` | HIRC section, Sound object and curve ordinals |
| `audio-bank-hirc-sound-rtpc-point` | `bank-hirc-sound-rtpc-point` | HIRC section, Sound object, curve and point ordinals |
| `audio-bank-hirc-state` | `bank-hirc-state` | HIRC section and standalone State object ordinals |
| `audio-bank-hirc-state-property` | `bank-hirc-state-property` | HIRC section, State object and property ordinals |
| `audio-bank-hirc-inline-state` | `bank-hirc-inline-state` | HIRC section and owning HIRC object ordinals |
| `audio-bank-hirc-inline-state-binding-count` | `bank-hirc-inline-state-binding-count` | HIRC section and owning HIRC object ordinals |
| `audio-bank-hirc-inline-state-binding` | `bank-hirc-inline-state-binding` | HIRC section, owning HIRC object and binding ordinals |
| `audio-bank-hirc-inline-state-group-count` | `bank-hirc-inline-state-group-count` | HIRC section and owning HIRC object ordinals |
| `audio-bank-hirc-inline-state-group` | `bank-hirc-inline-state-group` | HIRC section, owning HIRC object and group ordinals |
| `audio-bank-hirc-inline-state-entry` | `bank-hirc-inline-state-entry` | HIRC section, owning HIRC object, group and State ordinals |
| `audio-bank-hirc-inline-state-property` | `bank-hirc-inline-state-property` | HIRC section, owning HIRC object, group, State and property ordinals |
| `audio-bank-hirc-actor-mixer` | `bank-hirc-actor-mixer` | HIRC section and ActorMixer object ordinals |
| `audio-bank-hirc-actor-mixer-child` | `bank-hirc-actor-mixer-child` | HIRC section, ActorMixer object and child ordinals |
| `audio-bank-hirc-actor-mixer-rtpc` | `bank-hirc-actor-mixer-rtpc` | HIRC section and ActorMixer object ordinals |
| `audio-bank-hirc-actor-mixer-rtpc-curve` | `bank-hirc-actor-mixer-rtpc-curve` | HIRC section, ActorMixer object and curve ordinals |
| `audio-bank-hirc-actor-mixer-rtpc-point` | `bank-hirc-actor-mixer-rtpc-point` | HIRC section, ActorMixer object, curve and point ordinals |
| `audio-bank-hirc-node-properties` | `bank-hirc-node-properties` | HIRC section and owning Sound, ActorMixer or container object ordinals |
| `audio-bank-hirc-node-property` | `bank-hirc-node-property` | HIRC section, owning object, property list and original property ordinals |
| `audio-bank-hirc-node-fx` | `bank-hirc-node-fx` | HIRC section, owning Sound, ActorMixer or container object and effect list ordinals |
| `audio-bank-hirc-node-fx-slot` | `bank-hirc-node-fx-slot` | HIRC section, owning object, effect list and physical slot ordinals |
| `audio-bank-hirc-node-aux` | `bank-hirc-node-aux` | HIRC section and owning Sound, ActorMixer or container object ordinals |
| `audio-bank-hirc-node-positioning` | `bank-hirc-node-positioning` | HIRC section and owning Sound, ActorMixer or container object ordinals |
| `audio-bank-hirc-node-positioning-path` | `bank-hirc-node-positioning-path` | HIRC section, owning object, physical positioning table and row ordinals |
| `audio-bank-hirc-node-advanced-settings` | `bank-hirc-node-advanced-settings` | HIRC section and owning Sound, ActorMixer or container object ordinals |
| `audio-bank-hirc-container` | `bank-hirc-container` | HIRC section and container object ordinals |
| `audio-bank-hirc-container-child` | `bank-hirc-container-child` | HIRC section, container object and child ordinals |
| `audio-bank-hirc-random-sequence` | `bank-hirc-random-sequence` | HIRC section and Random/Sequence object ordinals |
| `audio-bank-hirc-random-sequence-playlist-item` | `bank-hirc-random-sequence-playlist-item` | HIRC section, Random/Sequence object and playlist item ordinals |
| `audio-bank-hirc-switch-container` | `bank-hirc-switch-container` | HIRC section and Switch object ordinals |
| `audio-bank-hirc-switch-membership-group` | `bank-hirc-switch-membership-group` | HIRC section, Switch object and membership group ordinals |
| `audio-bank-hirc-switch-membership` | `bank-hirc-switch-membership` | HIRC section, Switch object, membership group and member ordinals |
| `audio-bank-hirc-switch-child-configuration` | `bank-hirc-switch-child-configuration` | HIRC section, Switch object and child configuration ordinals |
| `audio-bank-hirc-blend-container` | `bank-hirc-blend-container` | HIRC section and Blend object ordinals |
| `audio-bank-hirc-blend-layer` | `bank-hirc-blend-layer` | HIRC section, Blend object and Layer ordinals |
| `audio-bank-hirc-blend-association` | `bank-hirc-blend-association` | HIRC section, Blend object, Layer and child association ordinals |
| `audio-bank-hirc-blend-point` | `bank-hirc-blend-point` | HIRC section, Blend object, Layer, child association and point ordinals |
| `audio-bank-hirc-blend-layer-rtpc` | `bank-hirc-blend-layer-rtpc` | HIRC section, Blend object and Layer ordinals |
| `audio-bank-hirc-blend-layer-rtpc-curve` | `bank-hirc-blend-layer-rtpc-curve` | HIRC section, Blend object, Layer and RTPC curve ordinals |
| `audio-bank-hirc-blend-layer-rtpc-point` | `bank-hirc-blend-layer-rtpc-point` | HIRC section, Blend object, Layer, RTPC curve and point ordinals |
| `audio-bank-hirc-container-rtpc` | `bank-hirc-container-rtpc` | HIRC section and container object ordinals |
| `audio-bank-hirc-container-rtpc-curve` | `bank-hirc-container-rtpc-curve` | HIRC section, container object and curve ordinals |
| `audio-bank-hirc-container-rtpc-point` | `bank-hirc-container-rtpc-point` | HIRC section, container object, curve and point ordinals |
| `audio-bank-music-segment` | `bank-music-segment` | HIRC section and segment object ordinals |
| `audio-bank-music-segment-child` | `bank-music-segment-child` | HIRC section, segment and child ordinals |
| `audio-bank-music-segment-stinger` | `bank-music-segment-stinger` | HIRC section, segment and stinger ordinals |
| `audio-bank-music-stinger-schedule` | `bank-music-stinger-schedule` | HIRC section, owning MusicSegment, MusicSwitch or MusicPlaylist object and stinger ordinals |
| `audio-bank-music-segment-marker` | `bank-music-segment-marker` | HIRC section, segment and marker ordinals |
| `audio-bank-music-track` | `bank-music-track` | HIRC section and track object ordinals |
| `audio-bank-music-track-source` | `bank-music-track-source` | HIRC section and track and source ordinals |
| `audio-bank-music-track-clip` | `bank-music-track-clip` | HIRC section and track and clip ordinals |
| `audio-bank-music-track-switch-member` | `bank-music-track-switch-member` | HIRC section and track and switch-member ordinals |
| `audio-bank-music-track-controls` | `bank-music-track-controls` | HIRC section and track object ordinals |
| `audio-bank-music-track-automation` | `bank-music-track-automation` | HIRC section, track object and automation ordinals |
| `audio-bank-music-track-automation-point` | `bank-music-track-automation-point` | HIRC section, track object, automation and point ordinals |
| `audio-bank-music-playlist` | `bank-music-playlist` | HIRC section and playlist object ordinals |
| `audio-bank-music-playlist-child` | `bank-music-playlist-child` | HIRC section and playlist and child ordinals |
| `audio-bank-music-playlist-stinger` | `bank-music-playlist-stinger` | HIRC section and playlist and stinger ordinals |
| `audio-bank-music-playlist-item` | `bank-music-playlist-item` | HIRC section and playlist and item ordinals |
| `audio-bank-music-transition-rules` | `bank-music-transition-rules` | HIRC section and owning MusicSwitch or MusicPlaylist object ordinals |
| `audio-bank-music-transition-rule` | `bank-music-transition-rule` | HIRC section, owning music object and rule ordinals |
| `audio-bank-music-transition-id` | `bank-music-transition-id` | HIRC section, owning music object, rule, ID list and ID ordinals |
| `audio-bank-music-switch` | `bank-music-switch` | HIRC section and switch object ordinals |
| `audio-bank-music-switch-child` | `bank-music-switch-child` | HIRC section and switch and child ordinals |
| `audio-bank-music-switch-stinger` | `bank-music-switch-stinger` | HIRC section and switch and stinger ordinals |
| `audio-bank-music-switch-group` | `bank-music-switch-group` | HIRC section and switch and group ordinals |
| `audio-bank-music-switch-node` | `bank-music-switch-node` | HIRC section and switch and physical node ordinals |
| `audio-bank-music-switch-target` | `bank-music-switch-target` | HIRC section and switch and traversal target ordinals |
| `audio-bank-dialogue-event` | `bank-dialogue-event` | HIRC section and DialogueEvent object ordinals |
| `audio-bank-dialogue-argument` | `bank-dialogue-argument` | HIRC section, DialogueEvent object and original argument ordinals |
| `audio-bank-dialogue-node` | `bank-dialogue-node` | HIRC section, DialogueEvent object and physical node ordinals |
| `audio-bank-hirc` | `bank-hirc` | HIRC section ordinal |
| `audio-bank-stid` | `bank-stid` | STID section ordinal |
| `audio-bank-name` | `bank-name` | STID section and name ordinals |
| `audio-bank-stid-tail` | `bank-stid-tail` | STID section ordinal |
| `audio-bank-failure` | `bank-failure` | Failure phase and section ordinal when known |

Each section row retains the original four tag bytes, including unknown and non-UTF-8 tags,
and its exact header and body ranges. A BKHD row retains its original version, bank ID,
language ID and reader version support. Ranges are half-open in decoded Entry coordinates.
Their starts, exclusive ends and payload ordinals use unsigned decimal strings.

Each original DIDX triplet retains its ID, authored DATA offset and length, and the
range of its declaration. Repeated media IDs are separate rows in original order. Every
repeat carries an informational `audio.bank.duplicate-media` issue linked to the first
and repeated ID words in that physical bank. DATA
resolution reports `resolved`, `missing`, `ambiguous`, `unavailable` or `outside-data`.
A resolved row links the exact DATA section and retained byte range. A bounded prefix
read supplies the observed media class, optional RIFF length and prefetch decision.
The class can be `wem`, `midi`, `impulse` or `unknown`.

HIRC object rows retain original order, raw type bytes, IDs and raw object ranges.
The version-specific kind is derived when a unique supported BKHD version is available.
The HIRC section summary retains its declared object count and exact trailing range,
including an explicitly empty range. The current Audio capability lists coverage for
decoded object fields and [embedded WEM metadata](media-audio-metadata.md#wems-embedded-in-soundbanks)
separately from these physical envelopes.

Action rows from that same physical object and observed bank header carry the original
action type and target ID. Version 154 Play actions also carry the original bank ID
(including zero) and bank type. Other versions and non-Play actions have a null PlayBank
value, with receipts for the decision that applied. Each word has its own original
byte range. Both skipped-property counts are selectors for the PlayBank fields.
An unsupported Action layout retains the original body and reports its layout explicitly.
A malformed Action contributes the fields read before the failure to diagnostic output.

Decoded Play and PlayEvent actions emit a target resolution with the original target ID.
Its `local` object classifies the current bank's physical HIRC candidates and reports their
observed and matching counts. Play admits any same-ID local object. PlayEvent requires an
Event and still lists same-ID objects of other kinds with `matches_target_kind: false`.
Candidate rows keep every occurrence in physical HIRC order. Each points to its original
`audio-bank-object` by section and object ordinal. An incomplete local HIRC inventory reports
`unavailable` and retains every observed candidate.

The `foreign` status is `local-only` for Play without a nonzero bank ID. A matching local
object takes precedence over a named bank or a remote Event search and produces `local-match`.
A foreign selector with a complete local miss produces `directory-search` with a
`query_index`. When the local scan cannot establish absence, the status is `unavailable`
with reason `incomplete-local-inventory`. The selector retains the authored bank ID or Event
route in either case.

Join `query_index` to `audio-bank-foreign-target-resolution` within the Action bank's
owning Archive. Use the Action source's root digest and containing Entry chain to find
the directory row's source. A root directory row has source identity `root`. A nested
directory row has the containing Archive Entry as its source. The resolution records the
referring bank's physical Entry ordinal, selector and target ID. Actions from the same bank
with the same selector and target share a query. Candidate and blocker rows join to the
same directory and query index.

Play considers foreign banks with the authored BKHD bank ID. PlayEvent considers Event
objects in the owning Archive and includes other same-ID HIRC kinds with
`matches_target_kind: false`. Physical candidates keep duplicate Entry paths and HIRC
IDs in directory and section/object order. Each row supplies the target Entry and BKHD,
HIRC section and HIRC object ordinals. It links to the original bank and object records,
which retain the raw payload and physical language placement.

The exporter scans selected banks' checked retained headers and HIRC envelopes. A
filtered, unreadable or ambiguous bank has a blocker when it could affect the query.
A bank with a proved different BKHD ID cannot affect explicit Play. Incomplete HIRC
retains the readable object prefix with a blocker. `directory_complete` describes the
observed directory, while `candidates_complete` additionally requires a complete search
for this query. Incomplete coverage gives `unavailable` even if candidates were observed.
A complete query gives `missing`, `resolved` or `ambiguous` by its count of matching
physical objects. Runtime playback applies its own bank and language selection.

Event rows preserve the original action count and its byte range. Versions through 122 use a
little-endian unsigned 32-bit count. Versions 123 through 154 use a Wwise variable-length count.
Each action ID is emitted in original order with its exact word range, including repeated IDs.
The resolution row reports the number of observed same-ID objects and the number of Action
matches in that bank. Its result is `resolved`, `ambiguous`, `missing`, or `unavailable`.
Each candidate row names the original section and object ordinals and type byte. A same-ID
object of a different kind remains visible with `action_candidate: false`. A malformed HIRC
inventory leaves observed candidates readable and resolution unavailable. Candidate rows retain
physical section and object order, with no per-reference inline candidate array.
A complete Event emits the exact residual body range. Diagnostic Event rows preserve readable
count fields and the actual count-prefix range. An unsupported layout retains its original body
and reports the version-dependent status explicitly.

Sound rows preserve the original plug-in ID, storage code, source ID, in-memory size and
source flag byte when present. Versions 151 through 154 also preserve the original cache ID.
Storage, playback and language flags retain the original words behind their interpretations.
Versions 150 through 153 expose the opaque range after the source fields. The optional
version 154 node retains its original parent ID, including zero, the original routing
counts and the word immediately before the parent. An admitted source plug-in block retains
its declared length and exact raw parameter range. A malformed Sound contributes its
successfully read fields and checked ranges to diagnostic output. An unsupported version
retains the original body and reports its layout explicitly.

For a decoded Sound with embedded or prefetched media, the resolution row classifies the
original source ID against the DIDX triplets of the same physical bank. Each candidate row
links to an `audio-bank-media` record by section and triplet ordinal. Repeated IDs retain
every occurrence in physical order. An incomplete DIDX inventory reports `unavailable` and
retains the observed candidate prefix. The linked DIDX row describes DATA availability and
prefetch extent. A resolved DIDX identity alone makes no claim about media readability.

Decoded Sound sources with streamed or prefetch-streamed storage link to each numeric loose WEM
with the authored media ID in the same opened Archive. Prefetch-streamed Sound also keeps its
independent DIDX reference to the embedded prefix. The loose summary reports `resolved` for one
physical match, `missing` for none and `ambiguous` for multiple matches in a complete inventory.
A recovered directory reports `unavailable` with the observed candidates and an
`incomplete-inventory` reason.

The decimal-string candidate key is `target_entry_index`. Join it to the `audio-loose-wem-candidate` row with
the same root and that physical Entry index. The Sound bank's ordered container chain identifies
the owning directory. An empty chain joins the root inventory. For a nested chain, its final
Entry identifies the inventory source and the preceding Entries identify its containing chain.
Candidate rows preserve directory order and duplicate paths. The inventory row provides original
path and language placement, export selection and payload access. The reference describes
physical candidates. A playback choice depends on the selected language and fallback rule.


Standalone State layouts are decoded for versions 128 through 145. The summary reports the
checked property bundle and the remaining opaque body range. Each property retains its
original ordinal, raw ID and IEEE-754 word with separate source ranges. The optional numeric
interpretation is null for non-finite words. Repeated and unknown IDs retain their own rows.
Other standalone versions report `unsupported` and retain the original body. A malformed
supported bundle reports `malformed` in diagnostics.

Version 154 Sound, ActorMixer and container inline State reports a checked region when present. Binding and group
counts have their own original variable-length field ranges. Ordered rows preserve authored
binding IDs and raw flags, group IDs and synchronization bytes, State IDs and property
bundles. A Sound with no node reports `absent`. A malformed earlier region reports
`unavailable`. A checked State region remains available in diagnostics if a following
Sound field is malformed. The separate Sound and raw object rows retain their own
coverage. These inline State layouts are qualified to version 154.

The version 154 Sound, ActorMixer and container RTPC summaries report the original curve count, the checked curve count
and the exact admitted range. Each curve row retains its IDs, original type, accumulation and
scaling bytes, and its original point count. Point rows preserve the input and output
binary32 words, their finite numeric interpretations and the original interpolation word.
Repeated curve IDs and unknown numeric codes retain their physical ordinals. An explicit
zero count describes a present empty chunk. A Sound without a node reports `absent`.
If the count is unreadable, the summary reports `unavailable`. If a later curve is malformed,
diagnostics retain the declared count and every complete earlier curve and point.
RTPC cursors read only version 154 node bytes within the checked extent.

ActorMixer rows retain the v154 shared node parent and original node prefix fields, declared
child count, checked child count and exact bank-relative extents. Each child retains its
source position and original ID, including repeats. A child count beyond the original body
marks the ActorMixer malformed, while complete child words and earlier State and RTPC
observations remain available. Unsupported bank versions retain the original bank and
an explicit unsupported layout. Child IDs are authored references. Their receipts use the
owning bank bytes and do not attribute target payloads to the original word.

Version-154 Play action property summaries include the original target and
fade flag bytes and checked ordinary and ranged lists. Physical list indices
0 and 1 identify ordinary and ranged values. Each property row preserves its
original ID and complete raw words. The summary independently reports
complete or malformed property coverage, including when a later bank reference
is truncated. Other Action groups report absent Play properties. Other bank
versions report unsupported property coverage. The Action target, PlayBank
and CLI/MCP inspection fields keep their established meanings.

Version-154 Random/Sequence, Switch and Blend containers use the same shared
node property, effect, auxiliary routing, positioning, advanced settings
and inline State records. Container RTPC curves and points have their own
namespaces. Shared group rows can remain complete in diagnostics when later
container-specific data is truncated. The original container row and child
rows retain their existing layout and coverage.

Version-154 Random/Sequence decision summaries preserve original loop counts and
modifiers, transition binary32 words, avoid-repeat count, mode codes, flags and
playlist count and checked range. Finite transition values retain their original
numeric unit. Nonfinite transitions retain their complete original bits. Playlist
rows are in authored order, with repeated play IDs, signed 32-bit weights and
original weight bits. Play references carry unchecked resolution. A truncated
item with a full play ID retains its ID and has a malformed layout. The summary
reports unsupported versions and preserves the readable decision prefix when a
supported body is truncated. Each original field and numeric conversion refers
to its checked physical bytes in the owning bank.

Version-154 Switch summaries preserve the original group code and ID,
default value ID, continuous-validation byte and both original table counts,
checked ranges and independent completion states. Known group codes identify Switch and State. Membership
groups preserve their authored order, value IDs and checked member lists.
Per-child configuration rows preserve flags, signed fade words and original
bits, in their authored order. Repeated IDs and zero values remain present.
Each checked member and configuration uses its owning bank for field origins.
A truncated table retains its checked rows and readable field prefixes. Child
references carry unchecked resolution, independent of the original child list.

Ordinary version-154 Blend records preserve the original Layer count and
continuous-validation byte. Each Layer carries its original crossfade RTPC
ID and type code, checked child associations and its own initial RTPC rows.
Association points preserve ordered input and output binary32 words and
original interpolation codes. Finite numeric interpretations retain original
units. Repeated IDs, unknown codes and explicit zeroes remain present. A
malformed association can retain readable words and its original declared
count. A complete initial RTPC region remains usable when later association
bytes are truncated. Raw custom version markers remain unsupported. Each row
uses the checked physical source in its owning bank.

Version-154 Sound, ActorMixer and container node property summaries include the original override bus
and direct parent words and the checked property and ranged modifier lists. Explicit zero
values remain present. Each list reports its declared count, the checked ID and raw word counts,
and original byte ranges. A malformed prefix retains independently readable IDs and words. An
absent optional Sound node and an unsupported bank version each have their own layout status.
A later State, RTPC or child failure leaves a completed property prefix available.

Version-154 Sound, ActorMixer and container effect declarations use list index 0 for initial
effects and 1 for metadata. A list records its original override byte, count and
checked slot extent. A nonempty initial effect list also carries its bypass-all
byte. Slot records follow authored order. Each preserves its original slot index,
effect reference and flags or share-set byte as those fields become readable.
Repeated indices, original zero references and unknown bits remain present.

Aux declarations retain the original flag byte, up to four readable user bus IDs
at their authored positions and the original reflections bus ID. A set user-bus
flag establishes a four-position array. Null positions identify unreadable words
in a truncated list. A cleared flag omits the array. A readable zero ID retains its
value and receipt. The `reference_resolution` value `unchecked` denotes an authored
reference whose target has not been inspected. Each ID receipt belongs to the
physical owning bank.

The summaries distinguish an absent optional Sound node, an unsupported bank
version, a region unavailable because an earlier read failed and a malformed
region with a checked prefix. Complete earlier declarations remain available when
a later region, State, RTPC or child list is malformed.

Version-154 Sound, ActorMixer and container positioning summaries include the original
positioning flags, panner and 3D position codes, the parent override and any
separate 3D flags. A selected automation block records its original path mode,
signed transition time, vertex and playlist counts, and checked table ranges.
Path table indices 0, 1 and 2 identify vertices, playlist items and automation
ranges. Rows preserve physical order, signed durations, original playlist
offsets and counts, and coordinate binary32 words. Finite coordinates also
carry their numeric value. A short table retains independently readable words
and reports a partial row when its first word fits.

Advanced settings summaries record the two original flag bytes, virtual queue
and below-threshold behavior codes, and the original instance limit. Each
checked value has its own owning-bank receipt. The summaries distinguish a
missing optional Sound node, an unsupported version, a region unavailable
because an earlier read failed and a reached region with truncated fields.

Property row index 2 selects list 0 for scalar properties and list 1 for ranged modifiers.
Index 3 is the property's original position within that list. Each ID retains an individual
row, including unknown and repeated IDs. Scalar, minimum and maximum words are original
unsigned 32-bit values with individual ranges and source receipts. Missing words are omitted.
The list summary carries the coverage status. These fields make no numeric unit or float
interpretation and use only the original owning bank as their payload source.

Container rows preserve the version 154 Random, Sequence, Switch and Blend layout, original
parent ID and declared child count. The Random/Sequence row carries the original mode byte.
Each child ID is emitted in authored order with its own word range, including repeated IDs.
The checked node range, admitted child extent and suffix range refer to the retained bank.
A malformed suffix retains the admitted child rows. A child count that exceeds the original
body retains the count and leaves the child extent absent. Earlier undecoded versions retain
their physical object rows. Later unsupported versions report their layout explicitly.
The opt-in `original_hirc_containers_and_children_reconcile` test checks
the projected rows and retained child words against the slice reader.

Version 154 MusicSegment rows carry the original parent, counts, tempo, meter, grid period,
grid offset and duration. Floating-point values keep their original IEEE-754 words and a
finite JSON value when available. Separate ordered rows retain child track IDs, stinger
trigger and segment IDs, and marker IDs, positions and names. Stingers expose the checked
extent of the original scheduling bytes after the IDs. Marker names carry their byte range,
original NUL position and UTF-8 status. A malformed name keeps the earlier fully read rows
and the original segment remains available through the retained bank.

Version-154 MusicSegment, MusicSwitch and MusicPlaylist stingers also emit one
shared scheduling record per physical occurrence. These records retain the
original sync code, cue filter hash, signed repeat-time word with its raw bits,
and segment look-ahead count. A truncated final stinger retains every readable
word from its trigger ID onward. Field receipts identify exact bytes in the
owning bank. The original stinger records retain their existing opaque ranges.

Version 154 MusicTrack rows carry the authored parent, list counts, raw track kind byte,
interpreted kind, and switch or state group when present. Separate rows preserve ordered
source occurrences, clips and switch members, including repeated IDs. Source rows retain
the original plug-in ID, storage code, cache ID, in-memory size and flags. Checked plug-in
parameter, automation and switch transition ranges refer to the retained bank. Clip rows
retain the authored track ID independently from the physical clip position and media source ID. Clip times
carry their original IEEE-754 words and a finite JSON value when available. A malformed tail
keeps the preceding complete rows. Other layouts retain the physical HIRC object.

Version-154 MusicTrack controls include the original flags, subtrack count when
clips are present, signed look-ahead and Switch fade, sync and cue words. The
automation and point rows preserve the authored clip indices, unknown type and
interpolation codes, original binary32 bits and finite interpretations. Each
list has an original count and checked extent. Readable prefixes survive
truncation in both transition and graph rows. Clip indices remain authored
positions. Field receipts identify exact bytes in the owning bank.

Version 154 MusicPlaylist rows retain the music node, ordered children and stingers, and
items in their original order. Each item preserves its original segment and item IDs,
following child count, play mode code, signed loop counts, weight, avoid-repeat count and
raw weight and shuffle selector bytes. Each item also carries its interpreted play mode,
selector booleans and zero-based nesting depth. The depth uses the prior original child counts.
Its `bnkHircPlaylistChildCounts` source use encodes the first count offset, count of preceding
words, byte stride, and byte width, in that order.
The transition rule count and checked complete-rule prefix locate the original
scheduling bytes in the retained bank. A truncated item leaves earlier complete rows
available and reports the malformed layout.

Version-154 MusicPlaylist and MusicSwitch transition summaries report the
original rule count, complete rule count and checked table range. Rule rows
retain the original source and destination controls and allocated transition
object fields. Signed times and offsets retain their original bits. ID rows
preserve all complete words in each authored list, including repeats and
sentinels. List index 0 selects source IDs and index 1 selects destination
IDs. A malformed table keeps the readable prefixes of its rule and ID lists.
Each row carries exact field origins in its owning bank.

Version 154 MusicSwitch rows retain the music node, ordered children and stingers, raw
continue-playback and decision mode bytes, argument group IDs and type codes, and the checked
transition rule extent. Physical node rows expose the original key, packed link word, weight
and probability in file order, including unvisited nodes. Target rows follow the checked
nonzero-target traversal in order, with physical node indices, levels and ancestor indices.
Repeated visits to a physical node remain separate. A malformed path retains its preceding
checked targets and available physical nodes. The original tree extent refers to the retained
bank and can be shorter than its declared size for a truncated tree.

Version-154 DialogueEvent summaries retain the original probability, argument
count, tree length, decision mode byte and checked tree extent. Argument rows
preserve the original planar ID and type tables in order. Recognized mode and
group values carry a conversion receipt for their original byte. An unknown
code stays visible alongside its raw value.

Physical node rows preserve each readable key, raw link word, weight and
probability in file order. A complete key identifies a truncated final node,
and the summary also locates any unreadable final word bytes. A readable link
has `link_interpretation: undetermined`. The raw word can encode a target ID
or a child index/count pair. The export preserves repeated words and all
physical nodes. The generic HIRC node property rows preserve scalar values and
ranged modifier bounds as original words. The DialogueEvent summary reports
the independent property list coverage. A misaligned tree with a readable
declared boundary can still locate the following property lists. Other
bank versions report unsupported DialogueEvent coverage and retain the raw
object envelope.

STID header rows retain the authored string type and name count. Each name row carries its
original ID and byte count, exact header and name ranges, original bytes and UTF-8 result.
Repeated IDs and empty names retain their ordinals. Valid UTF-8 preserves whitespace and NUL
characters. Invalid UTF-8 retains the validated prefix length and the invalid sequence length
when known. A successful name scan emits an exact residual range, which can be empty.
Diagnostic output for a truncated name retains the authored count and readable prefix.

The provider streams physical sections before reading the selected DIDX, HIRC and STID bodies.
The second checked pass uses the complete directory evidence to select a unique DATA
section. Repeated recognized sections produce diagnosed projection loss, and all original
occurrences are available in diagnostic output. An incomplete directory leaves unresolved
DATA selection unavailable. An out-of-range media declaration keeps its original triplet
and its diagnosis.

The field-origin companion binds exact uses to the actual physical Entry. A resolved media
class identifies the admitted signature and form used to classify it. A declared RIFF
length identifies its own consulted size word. The raw media range and its originating
DIDX declaration remain separate fields. Record status, directory metadata and payload
encryption retain their distinct meanings.

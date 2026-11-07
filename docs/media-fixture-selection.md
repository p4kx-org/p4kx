# Capability-based media verification fixtures

Real-Archive canaries choose exact preferred public fixtures through the test-only selector in
`p4kx-testkit::media_fixture`. Assembly's shared adapter lives in
`crates/p4kx-assembly/tests/support/media_fixtures/`; production Entity lookup is unchanged.

## Selection contract

Each case declares a public family, required Entity kind, geometry and LOD policy, optional item
size, Interior or audio capability, and docked-vehicle expansion policy. The family table contains
public preferences already used by these tests and documented class prefixes; it contains no
private replacement names, Record GUIDs or asset paths from an installed test build.

Candidates retain the GUID, original Record path and physical Record-table index. Retrieval checks
all three against that exact position. A different Record with the same GUID cannot replace it.
Preferred matching compares complete class names with ASCII case folding. Duplicate preferred classes fail. A present
preference with an unreadable source, missing required geometry or incompatible capability fails;
it does not trigger fallback.

Only absent preferences permit deterministic alternatives inside the same documented family.
Eligible alternatives are ordered by physical source identity and checked against the same
capabilities. An absent compatible family produces an explicit absent outcome. Every
later registered case still runs after a failed or absent case; an entirely absent run fails.
A selected conversion failure remains a failed case. Selection and conversion are separate gates.

Canonical trees are read and inspected once. The resolver consumes the supplied tree and an
observer retains original typed dependency and required-file failures. Audio-only cases keep the
original trigger References and use `AudioIndex::resolve` without decoding the Entity again or
building an unrelated model graph. Dangling source links and malformed canonical data retain their
separate meanings.

Requested LOD and actual available geometry levels remain separate. The existing nearest-lower or
lowest-level policy is explicit in reported capabilities. Main model and Interior cases stop at
docked vehicles; other Assembly matrices retain their declared expansion policy. Existing minimum
parts, mesh sharing, placement, fallback-texture, palette, audio-resolution and media-location
thresholds remain active. Each requested texture mode has its own case outcome.

## Case scope

The shared adapter covers representative model, texture/palette, layer, follow-up, Interior,
Wwise-resolution and streamed/localised Assembly matrices. Exact regression fixtures for port
changes, attachment placement, cache identity or a particular authored Loadout keep their exact
source requirement; replacing them would change the invariant they test. Character and armour
cases with specific authored Loadouts also keep those requirements.

Ship, coarse-texture and Interior CLI package canaries reuse the shared capability selector.
Each CLI class/path argument is independently resolved to the chosen physical Record index, GUID
and path before invocation, and the returned report must identify the same Entity. These matrices
run every case and retain their existing geometry, material, sharing and size thresholds. CHF,
armour Loadout and animation requirements remain exact. Assembly-only results do not prove
package/importer behaviour; synthetic, real-Archive and Blender results are recorded separately.

## Privacy and evidence

Typed errors and complete identities remain available internally. Real-canary boundaries print
static failure categories, case labels, selection reasons, fulfilled capabilities and aggregate
counts. They do not format private source-bearing errors, replacement identities or game values.
Synthetic regression assertions may print only their authored synthetic fixtures.

The synthetic selector/report suite covers deterministic fallback, ambiguity, absent scope,
present incompatible preference, later-case execution, selected-conversion failure and safe error
formatting. Adapter tests independently cover physical GUID aliases, canonical/field errors,
missing declared geometry, dependency target identity and actual XML read/parse failures.

Actual LIVE/PTU canaries are opt-in tests configured with `P4KX_DATA_P4K` and the normal key helper.
Run them with `TMP`, `TEMP` and an isolated `P4KX_FOLDERS_ROOT` under the authorised agent task
folder. A synthetic pass is not actual Archive coverage. Record source build, channel, completed,
absent and failed case counts with any remaining failure categories.

Affected consumer outputs also require the source importer in an isolated Blender 5.2 process:
record the importer/source and package hashes, hierarchy, bounds, rig, images and material
assignments, plus fixed-camera renders for changed visible behaviour. Import success alone does
not establish original-source completeness or in-game equivalence. The offered interactive
Blender session remains unchanged.

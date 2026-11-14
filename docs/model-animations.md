# Animations: `.caf` files and `.dba` databases

What the reader of `p4kx-model` assumes about the animation files and what the real Archives say
about it. The tests are `crates/p4kx-extraction/tests/real_archive/animations/` (`#[ignore]`d, run
with `P4KX_DATA_P4K`; no entry involved is encrypted, so they need no Archive key; they pass on
LIVE). Surveyed on 2026-10-21.

## What there is

| Files of LIVE | Count | Animations | Tracks | Keys |
|---|---|---|---|---|
| `.caf` | 54,148 | 54,148 | 7,589,974 | 489,309,827 |
| `.dba` | 548 | 15,910 | 1,383,250 | 71,194,388 |

All are `#ivo` files; none is encrypted. Every animation runs at 30 frames per second. The longest
`.caf` has 21,599 frames (12 minutes), the longest animation of a database 2,190. The largest
animation holds 425,242 keys, the largest database 6,751,431. 1,475 `.caf` files and 1,905
animations of databases are additive (see below). The five channels of the Archive hold the same
numbers within a few hundred files.

## Entity rig input discovery

Assembly's `discover_entity_rigs` visits a resolved Loadout's root, attached entities and landing
gear in declaration order. It retains original geometry selectors, CDF model references, the
selected physical skeleton and its primary chunk, and ordered animation parameter rows with
original positions. Repeated placements remain distinct even when they share one Archive Entry.
Each requested input distinguishes missing, ambiguous, key-required, wrong-key and other read
failure. Encryption is unknown for a request whose bytes could not be read, and follows the
observed input and its Archive ancestry after a successful read. Unread or invalid root inputs
leave child and gear discovery available.

The Rust `discover_entity_clips` API inventories CAF and DBA header declarations under the
resolved rig positions. Tracks-database links preserve the actual source read or why it could
not be selected. A folder directive links the physical CAF and DBA candidates in its subtree.
Candidates shared by different placements refer to the same inventoried Entry. A concrete CAF
alias resolves by its rooted path or its preceding folder in the same AnimationList. A relative
alias records the folder as a separate selector. Wildcards remain in the authored parameters.

The header inventory preserves the physical clip ordinal, original 48-byte metadata and DBA name,
independent timing, and each observed controller descriptor and encoding word. A malformed block
does not discard a separately readable metadata record. The inventory reads one Archive Entry at
a time without decoding animation key samples or generating a pose.

## Where the keys are

| Chunk | ID | Version | In |
|---|---|---|---|
| `AnimInfo` | `0x4733C6ED` | `0x901` | every `.caf` |
| `CafData` | `0xA9496CB5` | `0x900` | every `.caf` |
| `DbaData` | `0x194FBC50` | `0x900` | every `.dba` |
| `DbaMetadata` | `0xF7351608` | `0x902` | every `.dba` |

The reader refuses other versions of these chunks (`AnimationError::UnsupportedVersion`); no file
of the Archives has one. Animations in the older `CrCh` container (controller chunk 905 with
in-place streaming and negative offsets) are not read (`UnsupportedContainer`).

### The 48-byte record

`AnimInfo` is one record; `DbaMetadata` holds one per animation after its header (a `u32` count and
a `u32` that is 0, then `count` records, then the animations' names).

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | flags: bit 0 set for an additive animation, bit 1 set in most files (meaning unknown) |
| 4 | 2 | frames per second |
| 6 | 2 | number of controllers |
| 8 | 2 | 0 |
| 10 | 2 | 9 |
| 12 | 4 | 0 |
| 16 | 4 | end frame |
| 20 | 16 | rotation `x, y, z, w` (`f32`) of the start pose |
| 36 | 12 | translation (`f32`) of the start pose |

The end frame equals the frame of the last key in all 592 animations of the first sample and the
number of controllers equals that of the block header; the reader takes the end frame and the frame
rate from the record and ignores the start pose. A frame rate of zero is corrupt. The names of a
database are NUL-terminated paths like `animations/characters/human/male_v7/weapons/no_weapon/getup/nw_neutral_stand_getupback.caf`
in database order, directly behind the records (8 plus 48 times the count is a multiple of 8, so
the alignment Cryengine-Converter describes is automatic). An animation's name is the stem of its
path (or of the `.caf` file).

The flag bit 0 marks the additive animations: the 18 of the 11 databases of `male_v7\weapons\no_weapon` in the first sample
(`..._add.caf`, `..._additive.caf`) have first rotations that lie a median 0.00 to 0.21 rad from the identity and 0.4 to 3.1 rad
from the bind pose, while the 489 others lie 0.2 to 0.3 rad from the bind pose and 0.4 to 1.2 rad from the identity. The keys are
offsets to be added to another animation; glTF cannot say that, and written as local poses they collapse the skeleton in a
viewer. The export therefore skips them unless they are named and lists them as skipped with the reason "additive animation
— needs a base animation"; a named one is written as it is with a note that its pose needs a base (ADR-0046).

### Blocks and controllers

A `.caf` `CafData` chunk starts with one block, a `.dba` `DbaData` chunk with a `u32` (its size
less 4 or so; the reader ignores it) and then one block per animation, back to back, followed by
**one pool of keys** for all of them. A block is

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | `#caf` or `#dba` |
| 4 | 2 | number of controllers *n* |
| 6 | 2 | `0xFFFF` (`.caf`) or `0xAA55` (`.dba`) |
| 8 | 4 | a size that hints at the pool and is not a stride |
| 12 | 4 *n* | controller IDs |
| 12 + 4 *n* | 24 *n* | controller entries |

A controller entry is two halves of 12 bytes, rotations then translations: `u16` key count, `u16`
format (low byte the time format, high byte the key format), `u32` offset of the times, `u32`
offset of the keys. **Offsets are relative to the start of the controller entry** and point into
the pool, which times of several controllers may share. A half with a count of 0 is empty; the
reader drops a controller without any keys.

The second block of `getup.dba` starts 4,100 bytes after the first although the first declares a
size of 72,288, which shows that the size is no stride. The `u32` at the start of its `DbaData` is 438,545 against a
span of 438,552 bytes by the chunk table (the distance to the next chunk).

### Key times

The low byte of the format is `0x40` plus 0, 1 or 2:

| Code | Times |
|---|---|
| `0x40` | one byte per key |
| `0x41` | one `u16` per key |
| `0x42` | `u16` first frame, `u16` last frame, then a bitmap of last − first + 1 bits, least significant bit first, padded to 32 bits; a set bit is a key |

Frames count from the start of the animation and are strictly increasing in every track of the
Archives (the reader refuses a track that does not increase, and a bitmap that marks another number
of frames than the track has keys). Seconds are the frame over the frame rate of the record.

### Rotations

| Code | Keys |
|---|---|
| `0x80` | four `f32` `x, y, z, w` (18,608 controllers) |
| `0x82` | 48 bits, small tree (8,442,432 controllers) |
| `0x83` | 64 bits, small tree (207 controllers, all in `.dba` databases of vehicle and reload animations) |

A small tree stores three of the four components of a unit quaternion, the omitted one being the
largest in magnitude; it is rebuilt as positive (the quaternion or its negation, which is the same
rotation). Each stored component is
an unsigned integer over ±1/√2, `(step / max − ½) · √2`. The three components are stored in
ascending order of `x, y, z, w` without the omitted one, from the low bits up; the two bits above
the components give the index of the omitted one:

| Code | Component bits | Unused | Index of the omitted component |
|---|---|---|---|
| `0x82` | 15, 15, 15 (bits 0 to 44) | bit 45 | bits 46 and 47 |
| `0x83` | 21, 21, 20 (bits 0 to 61) | none | bits 62 and 63 |

How this was found out (the formats are not documented by the tools the issue lists): for `0x82` the
mean angle between consecutive keys of 2.5 million keys is 0.031 rad with this layout and at best
0.064 rad with the alternatives (index in bits 45 and 46, descending order, …); bit 45 is 0 in
all of them. For `0x83` the layouts with 20 or 21 bits for all three components, the index in bits
60 and 61 or 63 and 62, and every permutation of the widths 21, 21, 20 were tried on the 5,065
rotations of `male_v7/vehicles/grin.dba`; this one gives 0.015 rad between consecutive keys and 0.065 rad
median distance of the first key from the bind pose of `bhm_skeleton_v7.chr`, the next best 0.030 and
0.76. StarBreaker decodes unknown rotation formats as small tree 48 without a word; the reader
reports them (`CorruptAnimation::UnknownFormat`) instead. A key is normalised, and a key whose
dot product with the previous one is negative is negated, so interpolating takes the short way
(`q` and `−q` are the same rotation). About 0.5 % of the consecutive keys of `0x82` rotations jump
by more than 0.35 rad per frame; these are cuts in the data, not decode errors.

### Headers only

`AnimationHeader::read_caf` and `DbaHeaders::read` read the record, the block headers and the
controller entries and decode no key (ADR-0157). They give the name, path, frame rate, end frame,
additive flag, the controller IDs of the tracks with keys and the number of keys they declare;
`p4kx model info` lists animations with them. On all 70,058 animations of LIVE they agree with the
decoded animations in every one of these fields and in the duration (end frame over frame rate).
An unknown key format shows only when the keys are decoded.

### Translations

| Code | Keys |
|---|---|
| `0xC0` | three `f32` per key (900,276 controllers) |
| `0xC1` | 24-byte header, then three `u16` per key, interleaved (1,457,403) |
| `0xC2` | 24-byte header, then one array of `u16` per axis whose scale is not `FLT_MAX` (1,998,510) |

The header is a scale and an offset per axis, six `f32`; a value is `u16 · scale + offset`. In
`0xC2` an axis with the scale `f32::MAX` is constant at its offset and has no array (of 4,843
`0xC2` controllers in a sample of 135 files 3,684 had no varying axis, 880 had one and 279 had two, none had three;
all 24,332 `0xC1` controllers of the sample had three). The reader also treats such an axis of
`0xC1` as constant.

### Absolute keys, controller IDs

All rotations and translations are **absolute** and relative to the parent bone, like `Bone::relative`:
the first rotations of 547 animations of the male character (64,287 tracks) lie a median 0.225 rad from the
bind pose and the first translations a median 20 mm from it (6.5 mm for the standing idles; the
rest hold weapon poses); a delta would lie at the identity and at the origin. This contradicts
older notes that call positions deltas.

The controller ID of a track is the CRC-32 (IEEE, as `crc32fast::hash`) of the bone's name **as
the file spells it**, with its case: it equals `Bone::controller_id` for all 59,770 bones of the 2,566 files that read (the 566
`.chr` and the first 2,000 `.cga` files); 0 differ. Cryengine-Converter describes the
hash of the lower-case name; that fits only the 2 of 220 bones of `bhm_skeleton_v7.chr` whose names
are lower-case anyway. 100 % of the tracks of the 547 animations bind to a bone of the male
skeleton; the unbound ones of an animation belong to bones of other skeletons (face bones of the
same database, props).

## Beyond the chunk span (#273)

The `#ivo` chunk table has no sizes, so `ChunkFile::chunk_data` stops at the next chunk's offset.
StarBreaker found `.dba` key data behind that span. In the 548 databases of LIVE (the same on the
other four channels) **no** controller's time or key offset lies behind the end of the span of
`DbaData` (the keys are in the chunk, `DbaData` before `DbaMetadata`), and none lies behind the end of the file.
The reader nevertheless reads from the start of the chunk to the end of the file
(`ChunkFile::chunk_data_to_end`), as the layout allows the pool anywhere after the blocks and the
testkit's `TestDba::pool_after_metadata` writes it behind the metadata chunk, and bounds every
offset against that slice.

## What the reader refuses

A frame rate of zero, a block whose signature is not `#caf` or `#dba`, a block or key array that
reaches past the data, key times that do not increase, a bitmap whose set bits are not the key
count, a quaternion without length or a value that is not finite, a rotation, position or time
format it does not know, a database whose metadata lists more animations than it has bytes for or
whose names are fewer than its animations. An animation of a database that is refused is skipped
and reported; the others are read (`Dba::skipped`). The reader caps the keys one animation may
declare at 4,194,304 (the largest in the Archives has 425,242) and those of a database at
16,777,216 (6,751,431), counted from the controllers' headers before anything is allocated.
Reading all 54,148 `.caf` files and the 548 databases takes about 11 seconds on the development machine.

## glTF

See ADR-0046.

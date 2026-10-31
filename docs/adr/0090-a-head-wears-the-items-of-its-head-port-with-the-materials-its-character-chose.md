# A head wears the items of its head port with the materials its Character chose

ADR-0200 adds that what follows the head also follows its expressions.

Decided for issue #152, which builds on ADR-0084 (the shaped head), ADR-0079 (armour on the body
skeleton) and ADR-0080 (textures and palettes). Evidence: `p4kx chf glb` and the real-Archive tests
(`crates/p4kx-assembly/tests/real_archive/real_dressed_head.rs`) over the 40 CHF files the LIVE
Archive ships (`masculinedefault.chf`, `femininedefault.chf`, the mission givers and NPCs under
`Data\Libs\CharacterCustomizer\`): 38 hold a face and all 38 are dressed with no piece skipped and
a valid GLB; the other two are material and tattoo presets without DNA. No player's file was used.
The default male at LOD 0 with the textures at mip 1 is 9 parts, 14 materials, 20 images and 37 MB,
built and written in 0.54 s; the default female 8 parts, 40 MB, 0.50 s. The file layouts are format
knowledge from the MIT-licensed StarBreaker project and chf-editor's documented facts, checked
against the Archive.

## What a head wears

A CHF names the items of a Character as a tree of item ports, each port by the CRC-32C of its
lower-cased name and each item by the GUID of its Entity Record. The head port (`head_itemport`)
holds the eyes, hair, brows, lashes, beard and the universal scalp, in the order of the tree; ports
the tool does not know are named `port_<hash>`. `Head::dressed` makes a piece of each item that has
geometry for the body variant: the body's tag (`Male`, `Female`) selects the sub-geometry, which for
the eyes is nested one level deeper, below a node tagged `Protos_head`, so the sub-geometry of an
Entity is read through nested nodes, the nodes nested in a node carrying its tags as well as their
own, and a node's children are tried before the nodes nested in them. An Entity without geometry,
such as the brows of a bald Character, is no piece and no issue; an item the Archive does not have
is a failed piece, a skin it does not have `geometry_missing`.

The teeth are not in the CHF; the head rig's own `_teeth.skin` is worn with the head. The eyes and
teeth have DNA rigs of their own beside their skins and are shaped by the Character's DNA like the
head. Hair, brows, lashes, beard and scalp have no rig: they follow the shaped head. A vertex of
such a piece takes the average move of the head vertices within a tenth of a metre, weighted by
the inverse square of the distance, against an anchor that does not move and counts as much as a
head vertex at 5 cm, so that hair hanging far from the head stays where it was and the move fades
smoothly. The head vertices are the unshaped head as the model gives them, and their moves are
the shaped head's.

## Which materials

The CHF keeps a material per attachment. The attachment's name is the CRC-32 (not the CRC-32C) of
the lower-cased port name and `_skinitemport` (#274; the head, body, eyes, hair, beard and eyebrow
values are reproduced by the tests). Its base material is a GUID that the
`materialLookupTable` of the Character generation parameters (`SCharacterGenerationParams`) maps
to a `.mtl`; the file then replaces the library of the piece (the head of the default male takes
`male17_t1_head_material.mtl`, whose diffuse map carries that Character's skin). A GUID that is
in no table is listed (`material_choice_unknown`) and the piece keeps the library its Record or
model names; a zero GUID changes nothing. A sub-material is named by the CRC-32C of its
lower-cased name, a float parameter by the CRC-32C of the exact parameter name (`DyeAmount`,
`BaseMelanin`, ...), and the four colours the tool reads are known from the shipped Characters:

- `BodyColor` (`0xbd530797`) is the skin tone: sRGB bytes 81, 52, 40 of the default male are the
  linear 0.082, 0.034, 0.022 of the `SourceAverageColor` of the head map it uses;
- `EyeColor` (`0x442a34ac`) is the iris colour;
- `0x15e90814` is the hair dye and `0xa2c7c909` the tint of the natural hair: over the 40
  Characters the first is a varied saturated colour wherever `DyeAmount` is 1 and the second is
  white (254) in most. These two names are inferred, not documented.

All colours in the CHF are sRGB bytes and are converted to linear once.

## How they are applied

glTF has no skin, iris or hair shader, so each choice becomes the colour of its material and the
textures stay as they are:

- skin (`HumanSkin_V2`): a material with a diffuse map is multiplied by the ratio of the chosen
  skin tone to the map's `SourceAverageColor`, divided by its largest channel when that is above
  1 so that the hue stays; one without a map takes the skin tone;
- eyes (`Eye`): the same with `IrisColor`; the colour tints the whole eye texture, sclera
  included, because glTF cannot tint the iris alone;
- hair (`HairPBR`; the lashes' `Hair` shader is left as it is): the natural colour from the melanin (blond over brown to black) and the
  redness, times the tint, mixed with the dye by `DyeAmount`. Dye applies only to a library whose
  hair materials declare `DyeAmount`: the brows' shaders do not, so a dye in the choice of a
  brow, which the default Characters carry as a blue they never show, is not drawn. A hair
  material takes the first choice of its attachment that sets a number or a colour when no
  sub-material is named for it, because the choice of a beard or brow keeps the sub-material name
  of the style chosen before.

The choice's textures (blemish, makeup and tattoo maps) and every parameter not listed are not
drawn; the piece is noted `look_ignored` with their number. The shader mathematics (melanin, dye,
skin tone rebase) are approximated by these single colours; a viewer shows an approximation of the
game's face, not its pixels.

## Textures in the rigged GLB

`write_rigged_glb_with` takes a `TextureLoader` like `write_scene_glb_with`: the textures of all
pieces are made once, each distinct image embedded once, a texture that cannot be read keeps the
plain colour and is listed. Hair cards (`HairPBR`) have no diffuse map, only an opacity map as
`TexSlot1` (`_opac`): such a map becomes a white image with the opacity as alpha, the material's
colour is the colour of the hair, and the material is alpha-masked at 0.5. This holds for every
`HairPBR` material with such a map in the shared writers, so `entity glb` and `model glb` cut hair
cards out too (changelog fragment `152-changed`); a `HairPBR` material without an opacity map is
unchanged. The cut-out hair follows the shaped head with a kernel that reaches zero at 10 cm, so
a card crossing that distance gets no step. `p4kx chf glb` writes
the textures by default at mip 1, which keeps the default Characters at some 40 MB; `--texture-mip
0` and `--materials colours` choose otherwise.

## Considered options

- Draw the choices in the shader maths (Chiang melanin model, skin tone rebase per texel): needs
  the shader's texture-space logic, which glTF cannot hold, and the exact constants are not
  documented; a single colour per material is what a viewer can show.
- Match a sub-material by its name only: the beard and brow choices of every shipped Character
  carry the name of another style's sub-material, so they would never apply.
- Export the eyes and teeth unshaped: the eyes of a Character with another head shape would sit in
  the base head's sockets.
- Skin the hair to the facial bones: the hair follows the shape of the head and, with morph
  targets, its expressions (ADR-0200), and, since ADR-0299, it is bound to the facial joints.

# Armour takes the tint palette of each worn piece

Decided for issue #732. It extends ADR-0080 (palettes of parts) to Armour and follows the rule of
ADR-0195: a part keeps its own palette and inherits none.

## What the data says

Checked on the LIVE Archive, imported into Blender and compared with the in-game item render: the
Defiance Core Tactical (`slaver_armor_heavy_core_01_01_01`), worn by the Loadout
`Scripts/Loadouts/Character/Human/Armor/NPC/Criminal/Pyro/Criminal_Gang_Guard_Heavy_Boss.xml`, was
drawn with white plates; in the game it is dark gunmetal.

- `m_slaver_heavy_armor_core_01_01_01.mtl`, material `core_plate_m`, `BaseLayer3`:
  `TintColor="1,1,1" PaletteTint="1"`, which takes the primary entry of the tint palette.
- The item Record names the palette on its per-body sub-geometry (`Male`, `Female`), not on its main
  geometry. The default palette of the material library names no Record.
- `Armour::build` resolved no palette; only `Assembly::build_with_palettes` did, for ship parts, so
  the layer kept its white `TintColor`.

## Decision

The Outfit and character Entity paths use the palette declared on the selected worn geometry. If
that geometry declares no palette, they use the palette on the same item's main geometry. The
palette keeps its original selector and Record, and the worn geometry still supplies the model and
its selected material. A piece uses no palette from other pieces.

When the material uses a palette slot, the resolver tries the selected item palette and then the
material library's default. An explicitly declared item palette that fails to resolve produces a
`palette` issue before the library default is tried. In an Outfit, the Character's `Look` applies
after the tint palette. `Armour::build_with_palettes` resolves palettes for Outfits.

`armour glb`, `character package`, `chf glb` and the Characters tab use `build_with_palettes`.

## Consequences

Armour whose layers take a palette entry (`PaletteTint`) is drawn in the colours of its palette in
the GLB and in the Blender package. Armour whose layers take none, such as the CDS legacy medium
armour, is unchanged.

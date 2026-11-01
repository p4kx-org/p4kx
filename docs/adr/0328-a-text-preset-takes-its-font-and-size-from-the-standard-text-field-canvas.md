# A text preset takes its font and size from the standard text field canvas

Decided for issue #992. Amends ADR-0101 ("The Archive holds no font style or size for the presets
of text fields") and the open item "Text sizes" of ADR-0306.

## Context

A `WidgetTextField` names a preset in `labelProperties.style` (`Heading1` to `Heading6`, `Title1` to
`Title5`, `Body`). The composer found no font style or size for it, drew the default font and, where
the cascade set no size, 60 % of the box (`text style Heading1 names no font style in the Archive`).

## What was found, on LIVE with `p4kx dcb export`

- The preset is a tag: `UI.Style._Standard.ModularKit.Widgets.textfield.textstyle-<preset>`
  (`textstyle-heading1`, `textstyle-body`, ...).
- The canvas `modularkit/standard/widgets/textfieldwidgetstandard` holds one style entry per preset
  tag. Its default styles set a `FontSize` for every preset (`Heading1` 64, `Heading2` 48,
  `Body` 18) and the font style `blenderpro-book`. Its brand styles, keyed by the style Record of
  the brand (`s_aegs_hud`, `s_rsi_hud`, `s_gama_hud`, ...), override them: `Heading1` is 45 with
  `titilliumweb-regular` for `s_aegs_hud`, 60 with `blenderpro-bold` for `s_rsi_hud`, 60 with
  `outfit-regular` for `s_gama_hud`. Nothing refers to the canvas by Reference; the game applies it
  to every text field, as ADR-0111 has it for the standard widgets.
- The other places that select a preset tag are the styles of single screens and the modular-kit
  styles; they are ordinary entries of the cascade.
- A `BuildingBlocks_FontStyle` Record has a `scaleModifier` (Titillium Web 0.87, Outfit 0.89): the
  size a text is drawn at is its `FontSize` times that factor, so the font decides the size too.

## Decision

- A text field whose cascade sets no font style or no font size takes them from the entry of its
  preset in the canvas of the standard text field: the entry of the brand style that the canvas of
  the field picks (`Canvas::pick_brand`), over the default entry, field by field. Entries of the
  cascade, the inline styles and the overrides of an instance win over it.
- Only entries that select exactly one preset tag count. Other modifiers of those entries (colour,
  alignment, spacing) are not applied here.
- A preset that the canvas sets no size for is listed as `text style <preset> has no font size in
  the Archive: taken from the height of the widget`; one without a font style stays listed as
  before.

## Compared with the game

Renders of `m_mfd_screen` with `--var powerstate=1` and the power page, brand `aegs`: every text
field of the page already has a size from entries of its canvas (32 to 70), so the sizes do not
change; the font becomes Titillium Web at 0.87, narrower than the default, so the labels are
smaller on screen. Not compared: the owner's in-game screenshots are not part of the repository,
so the render was not set beside them in this change.

## Not done

The line spacing of the default entry, the colours the entries set and the preset fonts of other
widget types are not read.

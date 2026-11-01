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
  size a text is drawn at is its `FontSize` times that factor, so a preset's size is drawn with its
  font.

## Decision

- A text field whose style (cascade, inline style, override of an instance) sets no font size
  takes the size of its preset from the canvas of the standard text field. Its font style comes
  from the preset too, unless the style sets one. A field whose size is set keeps its font as it
  was: the preset's font goes with the preset's size, not alone.
- The entry is the one of the brand style `s_<code>_` of the standard text field itself, over the
  default entry, field by field. The brand is that of the screen and is looked up in the standard
  canvas, not in the canvas that holds the field, so a nested canvas without brand styles gets the
  brand's entry too; a brand the standard canvas lacks gets the default.
- Only entries that select exactly one preset tag count. Other modifiers of those entries (colour,
  alignment, spacing) are not applied here.
- A preset with no entry or no size is listed as `text style <preset> has no font size in the
  Archive: taken from the height of the widget`; a missing standard canvas is listed once as
  `standard text field not found in the Archive`.

## Compared with the game

Renders of `m_mfd_screen` with `--var powerstate=1` and the power page, brand `aegs`: every text
field of the page already has a size from entries of its canvas (32 to 70), so by the rule above
the page does not change; the tab bar keeps its look. The preset applies to fields without such a
size on other screens. Not compared: the owner's in-game screenshots are not part of the
repository, so no render was set beside them.

## Not done

The line spacing of the default entry, the colours the entries set and the preset fonts of other
widget types are not read.

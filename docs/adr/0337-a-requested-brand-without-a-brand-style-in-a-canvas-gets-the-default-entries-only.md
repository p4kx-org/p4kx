# A requested brand without a brand style in a canvas gets the default entries only

Decided for issue #993. Amends ADR-0089 ("the canvas's brand style of the requested manufacturer,
else its first") and the open item "Labels on a pale-blue background" of ADR-0306.

## Context

Labels were drawn on solid pale-blue chips on the Aegis SHIELDS page and on the RSI power page.

## What was found, on LIVE with `p4kx dcb export`

- The chip is not a style on the text. It is the background of the unnamed `DisplayWidget` that is
  the parent of the label (`Background` enabled, colour slot `Accent2` or `Accent1`); the text
  itself sets no background. No style entry of the RSI canvas, of the brand style `s_rsi_hud_2025`
  or of the text presets of the standard text field sets `EnableBackground` or `BackgroundColor`
  for it, and no binding touches it. The text of the chip carries an inline `FillColor` of black
  and the font `blenderpro-heavy`, so the chip and its dark text are drawn together by the data.
- The master canvas of the shields page (`mc_s_shields_master`) holds an entry `Set Canvas` per
  brand style: `s_rsi_hud_2025` selects `rsi_mc_s_shields`, `s_gama_hud_2026` selects
  `gama_mc_s_shields`, and its default entry selects `gen_mc_s_shields`. It has no style for
  `aegs`. `gen_mc_s_shields` has `s_aegs_hud` among its brand styles and draws the label without
  a chip.
- `Canvas::pick_brand` took the first brand style of a canvas when the requested code matched
  none. For `aegs` the master therefore took `s_rsi_hud_2025`, its entry chose the RSI canvas, and
  the RSI palette (`Accent2` is `#b8d3ef`) filled the chip. That is the pale-blue label of the Aegis
  SHIELDS page.

## Decision

- A requested brand code that a canvas has no brand style for gives that canvas no brand style:
  the shared styles and the default entries apply, as they do in the game for a brand the screen
  does not name. Without a requested code the first brand style is still used, so a screen
  rendered without `--brand` is unchanged.
- The pale-blue chips of the RSI power page stay: the data enables them, with `Accent2` and black
  text, and nothing disables them. This is the one place where the render may differ from the
  game; see below.

## Compared with the game

Before and after renders of the powered `m_mfd_screen`, page SHIELDS, brand `aegs`: the solid chip
and the oversized black `SHIELDS` text are gone; the page shows the generic shields canvas (a faint
panel for the run-time readout) and the tab bar. The Aegis power page and its tab bar are
unchanged.

Not compared: the owner's in-game screenshots are not part of the repository. If the RSI power
page shows no chips in the game, something the data does not carry (a run-time state) hides them;
that is an open question for the owner.

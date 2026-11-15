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
  the shared styles and the default entries apply. Without a requested code the renderer
  uses the first brand style.
- The renderer draws the RSI power page's source-enabled backgrounds. An authored style or supported
  state binding may change how a widget is drawn. The renderer uses the same rules for every
  brand and page.

## Render evidence

In the powered SHIELDS still, the generic Aegis canvas draws the faint readout panel and tab bar.
The archive-backed Aegis/RSI comparison uses the RSI SHIELDS chip as its positive control.

The historical LIVE inspection found the enabled parent background and no authored rule in the
inspected sources that removed it from the still. Recheck the selected power-page canvas, parent
and label fields, resolved palette, applicable styles and relevant binding inputs after an Archive
update. The renderer evaluates supplied variables and their at-rest defaults. The report lists
unsupported bindings and approximations. The owner's in-game state and the appearance of the
labels there remain unverified.

## Trace on the installed Archive

The ignored source test follows the canvas actually selected by the powered RSI MFD and uses the
original widget coordinates to check the parent Reference, enabled background, inline fill and
font. It verifies the palette slot of the winning style Record against the background emitted
by the painter. It also counts direct binding targets and local input References on each
candidate's visited ancestor chain. The same DataForge is used for the still at rest. The
test reports Build identity and aggregate counts.

Run the test with the ordinary installed-Archive fixture environment.

    cargo test -p p4kx-screens --lib the_rsi_power_source_trace -- --ignored --nocapture

The binding count does not evaluate every authored input chain or locate unsupported raw
selectors. The trace therefore verifies the offline paint and its physical sources while the
in-game state and a complete source account of potentially suppressing rules remain open.

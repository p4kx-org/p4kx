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

The source test traces the original Record, scene or library position, parent Reference,
enabled background, inline fill and declared font for the painted pairs. It verifies the
background against the palette slot of the winning style Record and checks for active background
style overrides. The original parent name is independent of this test. Installed LIVE and PTU
both contain named enabled DisplayWidget parents with the relevant palette slots.

The test checks authored candidates in each entered static POWER canvas against the observed
`Instantiated` and `IsActive` gates. It follows original local binding operations on the painted
pairs and their ancestors, classifies supplied and missing variable declarations, and matches
renderer omissions to their original operation positions. Both installed inputs preserve the
authored background in the powered still. With no supplied variables the POWER canvas is outside
the entered canvas set, so that state provides no equivalent label visit.

Run the test with the ordinary installed-Archive fixture environment.

    cargo test -p p4kx-screens --lib -- --ignored --exact --nocapture power_source_witness::the_rsi_power_source_trace_binds_painted_labels_to_original_parents_and_palette

The offline evidence covers entered canvases, evaluated styles and the observed operations.
Game state, inactive dynamic canvas routes and the exact appearance in the game remain unresolved.
The renderer retains the source-backed background and reports unavailable runtime inputs through
the ordinary diagnostics.

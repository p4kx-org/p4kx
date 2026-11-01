# Screen texts are glyph outlines of the font movie, placed by the style cascade

Decided for issue #536, on top of ADR-0089 (screens) and ADR-0045 (texts of movies).
Amended by ADR-0328: the presets of text fields take a font and a size from the standard text field.

## Decisions

- `p4kx-screens` draws `WidgetText` and `WidgetTextField` as glyph shapes of the `Scene`, one placed
  item per character with a shared, cached outline per font, glyph and colour. The fonts are
  those of `UI/fonts/Shared/fonts_en.gfx`, which the `UIElement` Record `uielements/buildingblocks`
  names as `fontFile`. A front-end reads the movie and passes `TextSources { fonts, localise }` to
  the composer, so the service crate never opens the Archive (ADR-0003). `p4kx-gfx` gains
  `FontFace` (metrics in ems, character lookup, outlines) and `localise_text`, which the movies
  use too.
- The font of a widget is its `BuildingBlocks_FontStyle`, from the widget or the cascade modifier
  `FontStyleRecord`. The Record names a `.slug` paint file, not a GFx font, so the font is found by
  the normalised file name, then by the family without its weight, then by the family of the Record
  name, preferring the font whose bold flag equals `isBold`; `scaleModifier` multiplies the size.
  The default font is `Blender Pro Medium`. Fonts have the size, letter and line spacing,
  alignment, word wrap, case and text of the widget, over which the cascade modifiers
  `FontSize`, `LetterSpacing`, `LineSpacing`, `WordWrap`, `AutoFontSize`, horizontal and vertical
  alignment, case and the localised `Text` apply. Font sizes are pixels, not canvas units, also in
  raw canvases.
- The text is the cascade `Text`, else the text a component parameter binds to the widget
  (`BindingsLocalizedField` over `BindingsLocalizedComponentParameter`, with the value the holding
  `WidgetCanvas` names in `paramInputValues`, else the default), else `locString` or the
  `labelProperties.label` of a text field. `LOC_EMPTY`, `LOC_UNINITIALIZED`, `LOC_PLACEHOLDER` and
  an empty text are filled in by the game at run time and are left out and listed. A key the language
  lacks is drawn as it is and listed; a character without a glyph is an empty box and listed.
- Text widgets of automatic size are as large as their text, so the layout measures them first.
- The Archive holds no font style or size for the presets of text fields (`Heading1`, `Body`, ...);
  such a field takes the default font and a size of 60 % of its box, and says so in the report.
- The Records give widgets a `layer`; siblings now paint in ascending layer, with the layout order
  unchanged. `FieldModifierColor` reads its `color` field, as the Records have it.
- Limits as ADR-0045: a text keeps 4096 characters (`text cut to 4096 characters`), a screen draws
  2^17 characters (`truncated`), and the placed glyphs count against the 2^17 shapes of ADR-0089.

## Not done

State-driven texts (data bindings of numbers, booleans and variables), HTML text, text of buttons
and edit boxes, rotation, clipping, wrapping flex containers and the font of other languages are not
drawn or evaluated and stay listed.

## Considered Options

- Reading the `.slug` fonts the Records name: a proprietary format that needs its own reader.
- Copying the layout of `p4kx-gfx` text fields: it is private to the movie composer and tied to
  `DefineEditText`; the screens need only a small line layout.

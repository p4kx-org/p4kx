# Imported fonts of UI movies resolve by export name

Decided for issue #275. About nine in ten text fields of the UI movies of the game name a font that
the movie does not define but imports (`ImportAssets`, `ImportAssets2`) from another movie, mostly
`fonts/Shared/fonts_en`. The imported ID is a local ID of the importing movie; the exporting movie
knows the font under an export name (`ExportAssets`). This ADR fixes how the two are joined.
ADR-0152 amends it: fonts are keyed by (URL, name), the byte budget counts declared lengths, and
English is the default, no longer the fixed, font language.

## Decision

- **`p4kx-gfx` knows nothing of the Archive.** It reads the imports into `Assets::imports` (the URL
  as the tag states it, then the local ID and export name of every asset, at most `MAX_IMPORTS`),
  sets `Font::export_name` from the `ExportAssets` tag of the movie that defines the font, and finds
  a font imported under a local ID through `TextOptions::fonts`, the fonts of other movies, by the
  export name. Those fonts are matched by name only (name, full name, export name, ignoring ASCII
  case); the ID a font has in its own movie is never used for the importing movie.
- **`p4kx-extraction` resolves the URLs.** `UiMovie::render` reads the exporting movies from the
  Archive for the imported IDs that a text of the movie uses, takes the fonts out of their assets
  and passes them in `TextOptions::fonts` under the name the importing movie imports them by.
  Movies are read once per render, cached by Entry, at most `MAX_IMPORTED_MOVIES` (16),
  `MAX_IMPORT_BYTES` (256 MiB decompressed) and `MAX_IMPORTED_FONTS` (256 fonts); a movie that
  exports the name under a local ID it imports itself is followed up to `MAX_IMPORT_DEPTH` (4)
  levels, and a cycle ends as `TooDeep`. The Entry list is scanned once per distinct URL and
  folder, not once per imported name. Movies on disk resolve nothing; `UiMovie::import_source`
  opens the movie of a URL for both origins, and for a movie on disk only a path that stays inside
  the folder of the movie (no root, drive or stream prefix, no `..` climbing out of it).
- **URLs are matched, not looked up.** The URLs are relative to the importing movie
  (`../../../fonts/Shared/fonts_en.swf`) but not always right for the folder the movie lies in, and
  some name a movie that only exists in the localisation folders (`HUD_Font_LocFont.swf`). The
  Entries with the file name of the URL, or its `.swf`/`.gfx` twin, are ranked: the exact path, the
  same tail below `Data\UI`, the same tail below `Data\Localization\english`, the same file in that
  folder, any file below `Data\UI`, then any other; the URL's own extension before the twin, then
  the path. Rendering tries the ranked Entries until an export resolves and applies the
  import resource budgets while reading them. A higher-ranked movie can lack the requested
  export even when its filename matches. `UiMovie::import_source` opens the first ranked Entry
  for a URL. English is the default language and other languages hold different glyph sets.
- **A font without outlines is a missing font.** Fonts that the exporter stripped (the
  `HUD_Font_LocFont` family draws from system fonts at run time) have glyph slots but no shapes;
  drawing them would make blank text without a report, so a font is usable only when at least one
  glyph has an outline, and otherwise the text is drawn as boxes and listed as `missing_font`.
- **Unresolved imports are reported, not fatal.** `UiRender::imports` lists the movies read and the
  imported names that were not found with the reason (`NotFound`, `Unreadable`, `NoSuchExport`,
  `TooDeep`, `OverBudget`; `not_found` and so on in JSON, with the message beside it); the texts keep drawing as boxes.

## Frames and labels

The walker keeps the display list at the end of every frame (`Timeline::frame_lists`), capped at
`MAX_FRAME_PLACEMENTS` (2^20, a frame counts as one more) placements and `MAX_FRAME_BYTES` (64 MiB,
placements with their names and filter data) per movie; frames beyond either are not kept and one
failed tag reports it (`TooManyFramePlacements`, `FrameListsTooLarge`). The bytes are counted
because a name or filter of 60 kB repeated over thousands of frames would otherwise cost gigabytes
from a movie of a few hundred bytes. A frame label carries the index of the frame it names. The filters
of a placement are kept by kind with their parameters as stored, not decoded; the compositor still
skips them. Compacted GFx fonts (`DefineCompactedFont`) keep their ID and glyph count only: the
fonts of the game are `DefineFont3`, and no LIVE movie holds one.

## Considered Options

- Matching imported fonts by the ID of the font in the exporting movie: wrong, the IDs of two
  movies are unrelated and collide.
- Building an index of all Entry paths per render: millions of Entries to answer one or two URLs;
  one parallel pass over the Entry list that compares file names is cheaper and keeps no state.
- Falling back to a font of the same name in another movie when an export has no outlines: fonts
  of one name differ in glyph set between movies, so it would draw other text than the game.

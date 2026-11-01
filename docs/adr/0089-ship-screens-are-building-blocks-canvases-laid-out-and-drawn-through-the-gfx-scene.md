# Ship screens are BuildingBlocks canvases, laid out and drawn through the GFx scene

Amended by ADR-0337: a requested brand without a brand style in a canvas gets no brand style, not
the first one.

Decided for issue #170 and the spike #276. The roadmap described ship screens as GFx/SWF movies.
They are not: a ship's MFDs, radar, annunciators and HUD are `BuildingBlocks_Canvas` Records of
DataForge, trees of widgets with sizing, flex layout, tags, style entries and palette slots. The
SWF movie of the UI owner, `UI/BuildingBlocks/assets/SWF/BuildingBlocks_root.swf` named by the
`UIElement` Record `uielements/buildingblocks`, is a host that holds ActionScript and fonts; a
second movie, `UI/fonts/Shared/fonts_en.gfx`, holds the fonts. Nothing a screen shows is a shape
of those movies. P4KX therefore reads the canvases, lays them out and draws them into the `Scene`
of ADR-0020, so the PNG and the SVG writers, their budgets and their listing of what was left out
(ADR-0030) are reused unchanged. ActionScript is never run.

## What was found, on LIVE with `p4kx dcb export` and `p4kx ui screens`

- An Entity names a screen by a Reference to a canvas anywhere in its Record: the default view of
  `UIBuildingBlocksEntityComponentParams` (`layers[].views[].component.canvas`, MFDs, the radar,
  the HUD and the projector), `PhysicalScreenParams.Screens[].canvas` (annunciators), `canvas` of
  `UICanvasDecalDescriptorEntityComponentParams` (name and serial decals) and
  `starMapParams.uiElementsCanvasGUID`. The screens of a ship are those of every Entity in its
  Loadout, at the port that holds them.
- The Avenger Titan has twelve screen references on LIVE. Three of them, with what they use:
  - the MFD `ships/displays/mfdscreens/.../general/m_mfd_screen`, 800 x 600, on the ports
    `Support_L`, `Support_R` and `MFD`: 14 canvases (`m_eng_mfdcontent`, `mc_s_self_master`,
    `mc_s_target_master`, `i_incomingcomms`, ...), 2 textures, 16 vector graphics, 12
    localisation keys, 0 movies;
  - the radar display `components/map/mapdisplaymaster`, 1920 x 1080, on the port `Radar`: 50
    canvases, 2 textures, 11 vector graphics, 19 localisation keys; its Entity also names
    `bb_screenradar_c_app_starmap` through the star map parameters;
  - the annunciator `physicalscreens/annunciator/h_eng_annunciator_master_left`, 1920 x 1080: 2
    canvases, 2 textures, 5 keys (`flightHUD_Label_WPN`, `hud_Cool`, `hud_Pwr`, `hud_Shld`,
    `hud_Thr`).
- A canvas is `size`, a flat `scene` array of widgets whose `parent` is a weak pointer to another
  element of the array (`scene[n]`; `library[n]` for templates of lists and clones), and the style
  sources `style`, `defaultStyles`, `brandStyles` and `embeddedStyles`. Nested canvases come from
  `WidgetCanvas.canvas`. Counts over the 5034 Records whose path holds `buildingblocks`: 3819
  canvases, 690 styles, 20868 display widgets, 7579 images, 5153 texts, 4288 canvas widgets, 4041
  custom shapes (SVG), 2861 cards, 1400 circles; sizing behaviours `Fixed`, `Percent`,
  `PercentOfX`, `PercentOfY`, `Auto` (and `PercentOfZ`, three times); layout policies
  `FlexContainer` (4579), `StackContainer` (336), `GridPackContainer` (31); `coordinateMethod`
  `useRaw` (3039), `auto` (381), `aspectOverridesWidth` (366), `aspectOverridesHeight` (33).
- Colours are palette slots: a `BuildingBlocks_ColorStyle` names a slot (`Base`, `Accent1`,
  `Bright`, ...) and the style's `colorStyles` array holds the colour at the position of the slot
  in the slot enum; the manufacturer's HUD style (`styles/s_<code>_hud`) and the canvases' own
  brand styles supply it. Most backgrounds, fills and strokes are not inline but set by style
  entries (`FillColor`, `BackgroundColor`, `EnableBackground`, `BorderColor*`, `Alpha`,
  `IsActive`) whose selectors are tags (73027 uses), parent and ancestor conditions, type and
  their combinations.
- A texture is `imagePath` with a `.tif` extension that the Archive holds as `.dds`, a vector
  shape is `svgPath` of a custom shape, texts are localisation keys (`locString`, `@key`).

## Decisions

- A new service crate `p4kx-screens` (ADR-0003) reads the canvases from `p4kx-dataforge`, resolves
  the Loadout with `p4kx-assembly` and composes into the `Scene` of `p4kx-gfx`, which draws it.
  `p4kx ui screens <archive> <entity>` and `p4kx ui render <archive> --screen <canvas>` are the
  CLI; `ui render` keeps its movie form.
- One module per widget type under `elements/`: `display` (the box with background and border of
  display widgets, cards, containers and canvas widgets), `canvas` (descending into a nested
  canvas), `circle`, `polygon` and `unsupported`. Layout (`layout`, `flex`), the style cascade
  (`style/`), colours and the scene painter are separate modules; no file passes 400 lines.
- Layout: width and height as fixed, percent of the parent, percent of the other axis of the
  widget, or the size of the parent for `Auto` (listed); margin, padding, the position offset and
  a flex container with direction, justification, alignment, gap and grow, rows and columns also
  reversed. Raw canvases (`useRaw`) count fixed values in canvas heights; the unit is an
  assumption, checked only against the HUD, whose root widget is 1.2 x 0.9 raw units, and is
  listed in the follow-up issue.
- Style cascade, lowest to highest: the default style, the manufacturer HUD style, per canvas
  the shared and the brand style (the canvas's brand style of the requested manufacturer, else
  its first), the entries of the shared styles, of the canvas's defaults, of the brand and its
  Record, the embedded entries, and the widget's own entries. Entries of an outer canvas apply to
  the widgets of the canvases nested in it. Selectors evaluated: tag (a tag within the named
  tag's subtree), any-of, all-of and not-tag, type, parent, ancestor, and all-of / any-of
  conditions; modifiers applied: fill, stroke, background, border colours and widths, enable
  background, active and alpha. The state at rest is drawn: state tags, animations, transitions
  and bindings do not run, so nothing that only a game state shows appears.
- A selector that is not evaluated (`Child`, `Count`, anything unknown) makes its entry not
  apply and is listed once per entry. A colour slot the palette lacks, a colour that is not solid
  or a slot, rotation, wrapping flex containers and the stack and grid layouts (drawn as overlays)
  are listed per widget type with the names of up to three widgets.
- Out of scope, listed and not drawn: texts, textures, vector graphics, lists, clones and widget
  templates, progress meters, entity and 3D widgets, particle, light and dust widgets, buttons and
  the modular components, pivot, anchor, rotation, overflow clipping, minimum and maximum sizes,
  gradient backgrounds and the rounded or chamfered corners. Texts need the fonts of
  `fonts_en.gfx` and the font styles of the canvases, textures the DDS decoder of ADR-0020's
  bitmaps, vector graphics an SVG reader; each is its own issue. ActionScript is never executed,
  and StarBreaker, the prior art, does not execute it either.
- The listing reads assets from the widgets of the screen and of every canvas it nests, with the
  guard that a canvas is visited once: canvases, textures (a `.tif` as the `.dds`), vector
  graphics, movies and localisation keys without the `@` (`LOC_EMPTY`, `LOC_UNINITIALIZED` and
  `LOC_PLACEHOLDER` are no keys), and the widgets by element type with whether they are drawn.
- Precedence, lowest to highest: the widget's own fields, then style entries in the cascade
  order above, so a style entry beats a widget field (`BackgroundColor` of an entry over the
  widget's background), and an entry of an inner canvas beats one of an outer canvas and, inside
  one canvas, a later source beats an earlier one; of equal rank the later entry wins.
- Limits (the Records are untrusted), set by `Limits` and reported as `truncated`: at most 2^18
  widgets visited or placed, hidden ones and nested canvases included, 2^26 steps of the cascade
  (entries tried plus selectors evaluated, over the whole screen), 2^17 shapes and a depth of 64
  widgets and canvases, as for DataForge. `Parent` and `Ancestor` selectors nested deeper than
  four and any selector nested deeper than eight are not evaluated and are listed. Style entries
  are parsed once per style Record and shared, a canvas's palette once per canvas. A canvas that
  contains itself is drawn once and listed, widgets whose parents form a cycle and a Reference
  that no Record answers are listed, a canvas side below 1 pixel or above 16384 is an error;
  sizes are checked and nothing indexes unchecked.
- Hologram rendering (#172) and the viewer's UI tab (#171) are untouched: they need the 3D
  renderer, and #167 (the 2D renderer) is the one this uses. #169, texts of movies, does not carry
  over: canvas texts need the font style cascade above.

## Considered Options

- Rendering the SWF movies of the UI owner: they hold no shapes of any screen, only ActionScript
  and fonts, so the result would be empty.
- Porting StarBreaker's UI crate (about 60000 lines) with its flat scene, cascade passes,
  linear-light compositing and Slug text: too large for the module-size rules and not needed to
  show where widgets lie and which colours they take; the pieces it has and this does not are the
  follow-up issues.
- Drawing the layout as a new image path: the `Scene` already is the backend-independent form the
  raster and the SVG writer share, so the screens reuse both with no second renderer.

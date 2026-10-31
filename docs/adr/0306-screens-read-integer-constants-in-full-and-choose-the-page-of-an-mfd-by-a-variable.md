# Screens read integer constants in full and choose the page of an MFD by a variable

Decided for issue #969. Amends ADR-0296 ("Comparison with the game"), ADR-0106 ("Compared with the
game") and ADR-0277 (the canvas a binding picks).

## Context

ADR-0296 and ADR-0106 compared the powered `m_mfd_screen` of the Avenger Titan with in-game
screenshots and found four differences: two large solid light-blue blocks at the lower corners, no
tab bar `< PAGE NAME >`, no way to choose one page, and a ship-diagram rectangle on the self-status
page. They left open whether the game fills the blocks differently or hides them by a tag.

## What was found, on LIVE with `p4kx dcb export` over the 191 Records below `mfdscreens`

- **The blocks were never meant to be there.** They were `shape_Indicator_Backdrop`, the backdrop
  of the turret layout (`gen_mc_s_trt_layout`), drawn through the self-status canvas
  `canvas_TurretMode`, which the Records instantiate only where `seatdashboard/currentmode` equals
  4. The comparison constant is an `Int64` field of the Record. The reader took the field as a
  float and did not know that width, so the constant read as 0, and `currentmode = 0` equalled it.
  Of the 192 integer comparisons in these Records, 134 in 51 canvases have a constant other than
  0; all were read as 0. With the constant read, the powered MFD instantiates the self-status
  modes that the state at rest selects and none of the others: the turret layout, the missile
  mode, the nav readout and the guns mode are gone, and with them the blocks. So neither a fill
  that differs nor a tag hides the blocks: the widget is not part of the page.
- **Page choice.** The MFD content `m_eng_mfdcontent` holds the page in `canvas_LandscapeMFDView`
  and `canvas_PortraitMFDView`, whose `CanvasReferenceRecord` is bound to the string variables
  `landscapecanvasguid` and `portraitcanvasguid` (the Record GUID of the page). Without them the
  canvas of the widget is `mc_s_self_master`. The pages are the `mc_s_*_master` canvases of
  `power`, `weapons`, `scanning`, `diag`, `shields`, `config`, `comms`, `target` (portrait),
  `self`, `ifcs` and `smp`.
- **Tab bar.** The `canvas_Header / Footer` widget is the bar `< PAGE NAME >`: a name in a bar and
  two arrows, drawn by `gen_mc_s_header` (Aegis and the other brands), or by `rsi_mc_s_header`
  with `WidgetText` and its own `Text` binding. The name is the localised variable `mfdviewname`,
  bound to `ParamInput0` of the text field (and handed to the canvas as `ParamInput0`); the label
  of the field is a design-time placeholder (`@ui_leaderboards_Loadout`).
- **Typed parameters.** A canvas widget hands parameters of several kinds that share the names
  `ParamInput0` to `ParamInput4` (the power master binds a boolean and a number to `ParamInput3`).
  The state kept one value per name, so the number replaced the boolean and the nested canvas fell
  back to its default, which switched a notification on. The kinds are separate parameters.
- **Arrows.** The arrows of the bar are `WidgetIcon` widgets whose colour comes from the entry
  `Arrow Colour`: a `CustomShape` whose parent has the tag `Text_Body`. An icon holds a custom
  shape the way a text field holds a text.

## Decision

- **Integer constants are read in every integer width** (`Int64` and `UInt64` too), by the
  reader's number function and by the switch exceptions of the graph.
- **A string variable names the canvas of a canvas widget.** A `CanvasReferenceRecord` bound to a
  game variable that the caller sets (`--var landscapecanvasguid=<record path or GUID>`) takes
  that canvas, read as `compose` reads a canvas name. A name that is no canvas, or ambiguous, is
  listed as `the input of the bound field CanvasReferenceRecord is not evaluated: <why>` and the
  widget keeps its own canvas. The canvases are read before the walk, so the shape of the tree
  they need is known. An unset variable is as before: `a canvas that only the game names at run
  time`.
- **A localised variable is the text of a text field.** `LocalizedVariable` is evaluated like a
  string variable; a `LocalizedField` of a text (`Text`, `ParamInput0` and so on) that evaluates to
  a text sets that text on the widget, which draws it in place of its label, and hands it to a
  nested canvas as a localised parameter. A `LocalizedComponentParameter` takes the text its canvas
  hands it, else its default, so a field whose parameter is the placeholder `@LOC_PLACEHOLDER`
  draws nothing, as a field filled in by the game does. A localised field whose input is not
  evaluated leaves the label as it is.
- **Parameters are told apart by kind** as well as by name, in the state of a canvas and when a
  widget merges its own parameters with the bound ones.
- **An icon holds a part**, as a text field does (ADR-0296): the entries that require the type
  `CustomShape` or name no type and select by the parent apply to its shape, so the arrows of the
  bar take their colour.
- **A runtime image has no background.** The picture the game renders replaces the box. The
  ship diagram (`Own Vehicle Hologram`) had the pale fill of `UI_Generic_Flag_01`, which drew a
  light-blue rectangle over the page.

## Compared with the game (owner screenshots of the Gladius and the Constellation, LIVE)

Renders of `m_mfd_screen` with `--var powerstate=1`, `--var landscapecanvasguid=<page>` and
`--var mfdviewname=<name>`, 800 to 1000 pixels wide on a black background, looked at beside the
screenshots; the Titan has no screenshot on disk, so its pages are compared with the written
description.

- **Aegis, default page (self status).** The two blocks and the rectangle are gone: the page
  draws its frame and nothing else, as the game draws no solid block. Its contents (GIMBAL and
  GROUP above the diagram, the three ammunition counts below it) come from run-time values and
  the 3D diagram, which stay undrawn.
- **Aegis, POWER MANAGEMENT.** The render draws the readout row, the `OUTPUT` and `BATTERY`
  blocks with their icons and the tab bar with an arrow at each end and `POWER MANAGEMENT` in a
  teal bar, as the Gladius shows it. It differs in the readout values (zero: the values are
  run-time), in the text sizes (see below) and in the vertical bars and pips, which come from
  lists with no rows at rest.
- **Aegis, SHIELDS.** The page draws the `SHIELDS` label and the bar; the ship silhouette and the
  `100` over `100` are a run-time picture and number.
- **RSI (Constellation), POWER MANAGEMENT.** The palette matches the screenshot: pale lavender
  arrows and text, orange `BATTERY` and `OFFLINE`, an angled darker footer with the page name in
  wide capitals. Not matching: the page name has no `«` and `»` (the buttons are
  `ComponentGeneralButton` widgets, which have no renderer), the labels sit on pale-blue
  rectangles (a background that the style sets on a text), and the red block with the warning
  triangle is missing.

## Still different from the game, not decided here

- **Text sizes.** `text style Heading1 names no font style in the Archive`, so the large labels
  of the power page (`OUTPUT`) are far too large; the tab bar text is right.
- **Labels on a pale-blue background** on the Aegis SHIELDS and the RSI power pages: the
  background belongs to the text widget in the Records; whether the game draws it is not known.
- **Buttons.** `ComponentGeneralButton*` has no renderer (the RSI `«` `»`, the arrows of the
  Aegis bar are drawn through their icons).
- **Run-time pictures and numbers** (silhouette, readouts, rows, pips).

## Guesses, unverified against the game

- That the pale fill of a runtime image is not drawn, or is the tint of the hologram and not a
  box fill. Both read the same in every screenshot (no rectangle); the data does not tell which.
  The render draws no fill.
- That a text field with a bound `ParamInput0` draws that value in place of its label. 94 of the
  text fields of the MFD Records that bind it have the label `@LOC_PLACEHOLDER` and the others a
  placeholder such as `Loadout` or `0`; the page name in the screenshots shows the value is drawn.
- That `mfdviewname` and the page canvases pair up as the tab bar of the game shows them
  (`POWER MANAGEMENT` for `mc_s_power_master`): the game sets both at run time; the names come
  from the screenshots.

## Considered Options

- Hiding the cast backdrops by a rule of the renderer: the data decides it, since the widget is
  not instantiated; a rule would have hidden it for the wrong reason and left the other modes.
- Keeping one typed value per parameter name: the notification of the power page and any canvas
  that hands a boolean and a number under one name break.
- Naming the page by a GUID only: the record path is the form a person can write; both are read.

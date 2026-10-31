// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use clap::{ArgGroup, Args, Subcommand};
use p4kx_extraction::Language;
use p4kx_extraction::gfx::{Background, CanvasSize, MAX_CANVAS_PIXELS, MAX_CANVAS_SIDE, Rgba};
use p4kx_screens::Variable;

use crate::cli_hologram::HologramArgs;
use crate::exit::EXIT_CODES_HELP;

/// Arguments of `p4kx ui`.
#[derive(Debug, Args)]
pub(crate) struct UiArgs {
    /// What to do with the UI movie.
    #[command(subcommand)]
    pub(crate) action: UiAction,
}

/// The actions of `p4kx ui`.
#[derive(Debug, Subcommand)]
pub(crate) enum UiAction {
    /// Lists the tags, shapes, morph shapes, bitmaps, fonts and sprites of a GFx or SWF UI movie
    /// with their IDs and export names, and the tags that are unknown or cannot be read.
    #[command(after_help = EXIT_CODES_HELP)]
    Assets(UiSource),
    /// Lists the `BuildingBlocks` screens of an Entity and of everything in its default Loadout,
    /// such as the MFDs and the radar of a ship, with the canvases, textures, vector graphics,
    /// movies and localisation keys each uses, the widgets by element type and the UI movies the
    /// Entities name.
    #[command(after_help = EXIT_CODES_HELP)]
    Screens(UiScreensArgs),
    /// Writes every bitmap of a GFx or SWF UI movie into a folder as `bitmap_<ID>.png` and every
    /// shape as `shape_<ID>.json` and morph shape as `morph_shape_<ID>.json`, in twips as the movie
    /// stores them. External images are decoded from the DDS texture they name, relative to the
    /// movie in the Archive or on disk, also with the extension .dds instead of the one given.
    #[command(after_help = EXIT_CODES_HELP)]
    Extract {
        /// The UI movie.
        #[command(flatten)]
        source: UiSource,
        /// The target folder; created if missing, existing files of the same name are replaced.
        #[arg(long, value_name = "FOLDER")]
        out: PathBuf,
    },
    /// Renders the first frame of a GFx or SWF UI movie, with its sprites, as a PNG, or as an SVG
    /// with `--svg`. Shapes with solid, gradient and bitmap fills and strokes are drawn with the
    /// transforms, colour transforms and masks of their placements; external images are decoded as
    /// `ui extract` decodes them. Static texts and text fields are drawn from the glyph outlines of
    /// the fonts of the movie, with their size, colour and alignment, as paths in an SVG; a
    /// character whose font or glyph is missing is drawn as an empty box, since no system font is
    /// used. Buttons and videos are skipped, filters and blend modes are not applied, and every
    /// element left out or drawn as a fallback is listed.
    ///
    /// With `--screen` it renders a `BuildingBlocks` canvas of the Archive instead (the screens of
    /// a ship, listed by `ui screens`): the layout of its widgets is computed from the Records
    /// and drawn through the same renderer. Texts are drawn as glyph outlines of the fonts of
    /// `fonts_en.gfx`, in the font, size, colour and alignment of the widget and the style
    /// cascade; `--lang` resolves their localisation keys. Clones, lists and library templates,
    /// flex, stack and grid containers, anchor, pivot, rotation, clipping and corners are laid out
    /// and drawn. Textures are drawn as bitmap fills with the scaling method, flips and colour of
    /// the widget, and SVG files as shapes with their fit, colour overlay and nine-slice. Widget
    /// types without a renderer, and every text, texture, vector graphic and layout feature left
    /// out or approximated, are listed in the report.
    #[command(after_help = EXIT_CODES_HELP, group(ArgGroup::new("text_source").multiple(true).args(["entry", "screen"])))]
    Render {
        /// The UI movie, or with `--screen` the Archive.
        #[command(flatten)]
        source: UiSource,
        /// Render this `BuildingBlocks` canvas of the Archive, by record path or GUID, instead of
        /// a movie.
        #[arg(long, value_name = "CANVAS", conflicts_with = "entry")]
        screen: Option<String>,
        /// The manufacturer code, e.g. `aegs`, whose colour style the canvas is drawn with;
        /// without a match the first brand style of the canvas is used.
        #[arg(long, value_name = "BRAND", requires = "screen")]
        brand: Option<String>,
        /// Set a game variable before the bindings of the screens are evaluated, as `NAME=VALUE`,
        /// e.g. `--var powerstate=1` for a ship that is powered on; every variable is zero at
        /// rest, so the MFD of a cold ship draws nothing. A name is the last segment of the path
        /// a binding reads (`powerstate` for `/seatdashboard/powerstate`), or the whole path when
        /// it holds a `/`. `CANVAS::NAME=VALUE` limits a setting to the canvas whose Record name
        /// is CANVAS (`mapdisplaymaster::isradar=1`); canvases of the same name are not told
        /// apart. Repeatable, at most 64; the last setting of a name wins and case is
        /// ignored. The value is read as a boolean (`true`, `false` or a number), a number or a
        /// text, as the binding takes it. A name that no canvas of the screen reads is reported,
        /// and the variables in force are listed in the report. The powered MFD shows the page
        /// that `--var landscapecanvasguid=<record path or GUID>` names (`portraitcanvasguid` in
        /// the portrait view) and writes `--var mfdviewname=<name>` in its tab bar.
        #[arg(
            long = "var",
            value_name = "NAME=VALUE",
            requires = "screen",
            action = clap::ArgAction::Append,
            value_parser = clap::value_parser!(Variable)
        )]
        variables: Vec<Variable>,
        /// The file to write, a PNG or with `--svg` an SVG; replaced if it exists.
        #[arg(long, value_name = "FILE")]
        out: PathBuf,
        /// Write an SVG document that scales without loss instead of a PNG: shapes, gradients,
        /// strokes, transforms, colour transforms and masks become SVG elements, bitmaps are
        /// embedded as PNG data, and the output is the same for the same input. The size sets the
        /// size of the document, and the same limits apply as for the PNG, and the document is at
        /// most 1 GiB. A bitmap fill that does not repeat is drawn once, without continuing its
        /// edge pixels, and a stroke painted with a non-repeating bitmap tiles it.
        #[arg(long)]
        svg: bool,
        /// The image size in pixels: `WIDTHxHEIGHT` scales the stage to fit and centres it,
        /// `WIDTH` alone keeps its aspect ratio; by default one pixel per pixel of the stage. At most
        /// 16384 a side and 33554432 pixels.
        #[arg(long, value_name = "SIZE", value_parser = parse_size)]
        size: Option<CanvasSize>,
        /// The colour under the frame: `transparent`, `movie` for the background colour the
        /// movie sets, or `#RRGGBB` or `#RRGGBBAA`.
        #[arg(long, value_name = "COLOUR", default_value = "transparent", value_parser = parse_background)]
        background: Background,
        /// The language that localisation keys (`@key`) of text fields resolve in: a code such as
        /// `en` or `de`, or the folder name of the Archive. Needs an ENTRY or `--screen`. For a
        /// movie, without it keys are drawn as they are; for a screen the default is English. A
        /// key the language does not hold is drawn as it is and listed. For a movie it also names
        /// the localisation folder in which fonts the movie imports are looked for after the
        /// folders of the UI, English by default.
        #[arg(long, value_name = "LANGUAGE", requires = "text_source", value_parser = clap::value_parser!(Language))]
        lang: Option<Language>,
    },
    /// Draws the hologram of a vehicle as a PNG: the Assembly of the Entity, with its Loadout, seen
    /// through the narrow lens of the `UIHoloVehicle_Config` Record of the game and drawn as a
    /// transparent tinted shell whose edges glow. The picture is drawn offscreen on the graphics
    /// card or, with --software, on the software adapter, so no window is needed. Textures and
    /// interiors are not drawn. Docked vehicles are not expanded, and a Loadout change to a port
    /// below one is not applied and warned about on stderr.
    #[command(after_help = EXIT_CODES_HELP)]
    Hologram(HologramArgs),
}

fn parse_size(text: &str) -> Result<CanvasSize, String> {
    let side = |text: &str| {
        text.trim()
            .parse::<u32>()
            .ok()
            .filter(|&side| (1..=MAX_CANVAS_SIDE).contains(&side))
            .ok_or_else(|| format!("`{text}` is no size from 1 to {MAX_CANVAS_SIDE} pixels"))
    };
    match text.split_once(['x', 'X']) {
        Some((width, height)) => {
            let (width, height) = (side(width)?, side(height)?);
            if u64::from(width) * u64::from(height) > MAX_CANVAS_PIXELS {
                return Err(format!(
                    "{width} x {height} is more than {MAX_CANVAS_PIXELS} pixels"
                ));
            }
            Ok(CanvasSize::Fit { width, height })
        }
        None => side(text).map(CanvasSize::Width),
    }
}

fn parse_background(text: &str) -> Result<Background, String> {
    match text {
        "transparent" => return Ok(Background::Transparent),
        "movie" => return Ok(Background::Movie),
        _ => {}
    }
    let invalid = || format!("`{text}` is no colour: use transparent, movie, #RRGGBB or #RRGGBBAA");
    let hex = text.strip_prefix('#').ok_or_else(invalid)?;
    if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    let channel = |at: usize| {
        hex.get(at..at + 2)
            .and_then(|digits| u8::from_str_radix(digits, 16).ok())
    };
    match (channel(0), channel(2), channel(4)) {
        (Some(red), Some(green), Some(blue)) => Ok(Background::Color(Rgba {
            red,
            green,
            blue,
            alpha: channel(6).unwrap_or(u8::MAX),
        })),
        _ => Err(invalid()),
    }
}

/// Arguments of `p4kx ui screens`.
#[derive(Debug, Args)]
pub(crate) struct UiScreensArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// The Entity: its record path, GUID or class name.
    pub(crate) entity: String,
}

/// Where a UI movie is.
#[derive(Debug, Args)]
pub(crate) struct UiSource {
    /// A Data.p4k Archive when ENTRY is given; otherwise the GFx or SWF file.
    #[arg(value_name = "ARCHIVE_OR_FILE")]
    pub(crate) path: PathBuf,
    /// Path of the GFx or SWF Entry in the Archive, with `\` or `/` as separator, ignoring case.
    pub(crate) entry: Option<String>,
}

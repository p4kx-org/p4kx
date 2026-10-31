// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Screens of Star Citizen Entities: the `BuildingBlocks` canvases their UI components name, read
//! from the DataForge Records and rendered as stills through the GFx renderer (ADR-0089).
//!
//! [`ScreenComposer::screens_of`] lists the screens of an Entity and of everything in its
//! resolved Loadout with the UI assets each uses. [`ScreenComposer::compose`] lays a canvas out
//! and draws its widgets into a [`Composed`] scene of the canvas size, which
//! [`Composed::rasterize`] draws as an image and [`Composed::write_svg`] writes as SVG.
//! The colour slots of a widget take their colours from the brand style of the canvas.
//!
//! Layout follows the sizing, bounds, margin, padding, anchor, pivot, turn and scale of each
//! widget and the flex, stack and grid containers that arrange children; clones and lists repeat
//! the library templates of a canvas, overflow clips, and raw canvases count in pixels of their
//! own size (ADR-0106). Every widget type has its own module under `elements`; a type without a
//! renderer, and every feature the layout or the painters approximate, is listed in the
//! [`Report`] of the [`Composed`] scene. `ActionScript` is never run and no texture is drawn.
//!
//! The style entries of a screen are evaluated with the selectors of tags, types, parents,
//! ancestors (with their break conditions), children and counts, and their modifiers change
//! colours, geometry and flex settings. A screen is drawn in its state at rest: the bindings of
//! each canvas, calculated with every game variable at zero and the parameters its canvas widget
//! hands it, decide which widgets are instantiated and active and what their alpha is
//! (ADR-0110). [`ScreenComposer::with_variables`] sets game variables to other values, such as
//! `powerstate` to 1 for a screen that is on (ADR-0259); the [`Report`] lists them. A variable
//! that holds a canvas Record path or GUID, such as `landscapecanvasguid` of the MFD, picks the
//! canvas of the canvas widget bound to it, and a localised variable such as `mfdviewname` is the
//! text of the text field bound to it (ADR-0306).
//!
//! Texts are drawn as glyph outlines of the fonts in [`TextSources`], which a caller reads from
//! `fonts_en.gfx`: the font style of a widget and the cascade pick the font, size, letter and line
//! spacing, alignment and case, localisation keys resolve in the language of the caller, and a
//! character without a glyph is an empty box. Without [`TextSources`] text widgets are listed.
//!
//! Textures and vector graphics are read through [`ImageSources`]: image widgets draw their
//! texture as a bitmap fill with the scaling method, flips and colour of the widget, and icons,
//! custom shapes and images that name an SVG draw its paths as shapes, fitted to the widget, with
//! a colour overlay or palette slots and an optional nine-slice. The files are untrusted and
//! bounded by [`Limits`]; one that is missing or unreadable is listed and its widget skipped.
//! Without [`ImageSources`] these widgets are listed.
//!
//! A separator draws the `ModularKit` canvas of its direction and style (ADR-0111). What the game
//! fills at run time, such as a map picture, 3D content or a line to a placed widget, keeps its
//! box and is listed with its reason.
//!
//! [`Still::render`] does all of that for a front-end that hands it the sources: a [`Still`]
//! holds the PNG pixels or the SVG document of one canvas with its report, which
//! [`Still::encode`] turns into the bytes of a file.

mod assets;
mod bindings;
mod boxes;
mod canvas;
mod colour;
mod compose;
mod context;
mod corners;
mod elements;
mod error;
mod fit;
mod flex;
mod font_file;
mod geometry;
mod glyphs;
mod images;
mod inspect;
mod layout;
mod limits;
mod look;
mod paint;
mod paint_state;
mod picture;
mod reader;
mod report;
mod rest;
mod screens;
mod statics;
mod still;
mod style;
mod svg;
mod text;
mod transform;
mod variables;
mod walk;
mod walk_canvas;
mod walk_instance;
mod walk_measure;
mod walk_notes;
mod walk_part;
mod walk_rest;
mod walk_style;
mod walk_text;
mod widget;

pub use assets::ScreenAssets;
pub use compose::{Composed, ScreenComposer, TextSources};
pub use error::ScreenError;
pub use images::{ImageSources, LoadTexture, LoadVector};
pub use inspect::ElementCount;
pub use limits::Limits;
pub use report::{Report, Unsupported, VariableInForce};
pub use screens::{EntityScreens, MovieUse, Screen};
pub use still::{Still, StillFormat, StillRequest};
pub use svg::MAX_SVG_BYTES;
pub use variables::{
    MAX_NAME_BYTES, MAX_VALUE_BYTES, MAX_VARIABLES, Variable, VariableError, check_variable_count,
};

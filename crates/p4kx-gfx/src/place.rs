// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::GfxError;
use crate::geometry::{Matrix, Rgba};
use crate::reader::TagReader;
use crate::tag::Tag;

const PLACE_MOVE: u8 = 0x01;
const PLACE_HAS_CHARACTER: u8 = 0x02;
const PLACE_HAS_MATRIX: u8 = 0x04;
const PLACE_HAS_COLOR_TRANSFORM: u8 = 0x08;
const PLACE_HAS_RATIO: u8 = 0x10;
const PLACE_HAS_NAME: u8 = 0x20;
const PLACE_HAS_CLIP_DEPTH: u8 = 0x40;
const PLACE3_HAS_FILTERS: u8 = 0x01;
const PLACE3_HAS_BLEND_MODE: u8 = 0x02;
const PLACE3_HAS_CACHE: u8 = 0x04;
const PLACE3_HAS_CLASS_NAME: u8 = 0x08;
const PLACE3_HAS_IMAGE: u8 = 0x10;
const PLACE3_HAS_VISIBLE: u8 = 0x20;
const PLACE3_HAS_BACKGROUND: u8 = 0x40;

/// A colour transform: each channel is multiplied, then the addend is added, and the result is
/// clamped to 0–255.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorTransform {
    /// The factors of red, green, blue and alpha.
    pub multiply: [f32; 4],
    /// The addends of red, green, blue and alpha.
    pub add: [f32; 4],
}

impl ColorTransform {
    /// The transform that changes nothing.
    pub const IDENTITY: Self = Self {
        multiply: [1.0; 4],
        add: [0.0; 4],
    };

    /// Returns the transform that applies `inner` first and then `self`.
    #[must_use]
    pub fn compose(&self, inner: &Self) -> Self {
        let mut combined = *inner;
        for (((multiply, add), outer_multiply), outer_add) in combined
            .multiply
            .iter_mut()
            .zip(combined.add.iter_mut())
            .zip(self.multiply)
            .zip(self.add)
        {
            *multiply *= outer_multiply;
            *add = add.mul_add(outer_multiply, outer_add);
        }
        combined
    }

    /// Returns whether the transform changes no colour.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }

    /// Returns `color` transformed.
    #[must_use]
    pub fn apply(&self, color: Rgba) -> Rgba {
        let channel = |value: u8, index: usize| {
            let multiply = self.multiply.get(index).copied().unwrap_or(1.0);
            let add = self.add.get(index).copied().unwrap_or_default();
            let result = f32::from(value).mul_add(multiply, add).clamp(0.0, 255.0);
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "the value is clamped to 0–255"
            )]
            let byte = result.round() as u8;
            byte
        };
        Rgba {
            red: channel(color.red, 0),
            green: channel(color.green, 1),
            blue: channel(color.blue, 2),
            alpha: channel(color.alpha, 3),
        }
    }
}

/// A definition placed on a timeline at a depth, as it stands after the first frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    /// The depth; higher depths are drawn above lower ones.
    pub depth: u16,
    /// The ID of the definition placed.
    pub id: u16,
    /// The transform from the definition into its timeline.
    pub matrix: Matrix,
    /// The colour transform.
    pub color_transform: ColorTransform,
    /// The morph ratio from 0 (start) to 65535 (end).
    pub ratio: u16,
    /// The instance name.
    pub name: Option<String>,
    /// For a mask: the highest depth it clips.
    pub clip_depth: Option<u16>,
    /// The blend mode of `PlaceObject3`; 0 and 1 mean normal.
    pub blend_mode: u8,
    /// The filters `PlaceObject3` gives the placement, in order.
    pub filters: Vec<Filter>,
    /// Whether the placement is shown.
    pub visible: bool,
}

impl Placement {
    /// Returns the bytes a copy of the placement occupies, heap data included.
    pub(crate) fn cloned_bytes(&self) -> usize {
        let name = self.name.as_ref().map_or(0, String::len);
        let filters = self.filters.iter().fold(0usize, |sum, filter| {
            sum.saturating_add(size_of::<Filter>())
                .saturating_add(filter.data.len())
        });
        size_of::<Self>()
            .saturating_add(name)
            .saturating_add(filters)
    }
}

/// What one `PlaceObject` tag states about a depth.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PlaceCommand {
    pub(crate) depth: u16,
    pub(crate) id: Option<u16>,
    pub(crate) moves: bool,
    pub(crate) matrix: Option<Matrix>,
    pub(crate) color_transform: Option<ColorTransform>,
    pub(crate) ratio: Option<u16>,
    pub(crate) name: Option<String>,
    pub(crate) clip_depth: Option<u16>,
    pub(crate) blend_mode: Option<u8>,
    pub(crate) filters: Option<Vec<Filter>>,
    pub(crate) visible: Option<bool>,
}

impl PlaceCommand {
    const fn new(depth: u16) -> Self {
        Self {
            depth,
            id: None,
            moves: false,
            matrix: None,
            color_transform: None,
            ratio: None,
            name: None,
            clip_depth: None,
            blend_mode: None,
            filters: None,
            visible: None,
        }
    }

    /// Applies the command to the placement at its depth, `None` when the depth is empty.
    pub(crate) fn apply(
        self,
        current: Option<Placement>,
        observe: impl FnOnce(bool, &Self),
    ) -> Option<Placement> {
        let (mut placement, retained) = match (current, self.id) {
            (Some(current), None) if self.moves => (current, true),
            (Some(current), Some(id)) if self.moves => (Placement { id, ..current }, true),
            (_, Some(id)) => (
                Placement {
                    depth: self.depth,
                    id,
                    matrix: Matrix::IDENTITY,
                    color_transform: ColorTransform::IDENTITY,
                    ratio: 0,
                    name: None,
                    clip_depth: None,
                    blend_mode: 0,
                    filters: Vec::new(),
                    visible: true,
                },
                false,
            ),
            (current, None) => return current,
        };
        observe(retained, &self);
        if let Some(matrix) = self.matrix {
            placement.matrix = matrix;
        }
        if let Some(color_transform) = self.color_transform {
            placement.color_transform = color_transform;
        }
        if let Some(ratio) = self.ratio {
            placement.ratio = ratio;
        }
        if self.name.is_some() {
            placement.name = self.name;
        }
        if self.clip_depth.is_some() {
            placement.clip_depth = self.clip_depth;
        }
        if let Some(blend_mode) = self.blend_mode {
            placement.blend_mode = blend_mode;
        }
        if let Some(filters) = self.filters {
            placement.filters = filters;
        }
        if let Some(visible) = self.visible {
            placement.visible = visible;
        }
        Some(placement)
    }
}

/// Reads a `PlaceObject` (code 4), `PlaceObject2` (26) or `PlaceObject3` (70) tag.
pub(crate) fn read_place(tag: &Tag<'_>) -> Result<PlaceCommand, GfxError> {
    let mut reader = tag.reader();
    if tag.code == 4 {
        let id = reader.u16()?;
        let mut command = PlaceCommand::new(reader.u16()?);
        command.id = Some(id);
        command.matrix = Some(reader.matrix()?);
        if reader.remaining() > 0 {
            command.color_transform = Some(read_color_transform(&mut reader, false)?);
        }
        return Ok(command);
    }
    let flags = reader.u8()?;
    let flags3 = if tag.code == 70 { reader.u8()? } else { 0 };
    let mut command = PlaceCommand::new(reader.u16()?);
    command.moves = flags & PLACE_MOVE != 0;
    let has_character = flags & PLACE_HAS_CHARACTER != 0;
    if flags3 & PLACE3_HAS_CLASS_NAME != 0 || (flags3 & PLACE3_HAS_IMAGE != 0 && has_character) {
        reader.c_string()?;
    }
    if has_character {
        command.id = Some(reader.u16()?);
    }
    if flags & PLACE_HAS_MATRIX != 0 {
        command.matrix = Some(reader.matrix()?);
    }
    if flags & PLACE_HAS_COLOR_TRANSFORM != 0 {
        command.color_transform = Some(read_color_transform(&mut reader, true)?);
    }
    if flags & PLACE_HAS_RATIO != 0 {
        command.ratio = Some(reader.u16()?);
    }
    if flags & PLACE_HAS_NAME != 0 {
        command.name = Some(reader.c_string()?.into_owned());
    }
    if flags & PLACE_HAS_CLIP_DEPTH != 0 {
        command.clip_depth = Some(reader.u16()?);
    }
    if flags3 & PLACE3_HAS_FILTERS != 0 {
        command.filters = Some(read_filters(&mut reader)?);
    }
    if flags3 & PLACE3_HAS_BLEND_MODE != 0 {
        command.blend_mode = Some(reader.u8()?);
    }
    if flags3 & PLACE3_HAS_CACHE != 0 {
        reader.u8()?;
    }
    if flags3 & PLACE3_HAS_VISIBLE != 0 {
        command.visible = Some(reader.u8()? != 0);
    }
    if flags3 & PLACE3_HAS_BACKGROUND != 0 {
        reader.rgba()?;
    }
    Ok(command)
}

/// Reads a CXFORM, or with `alpha` a CXFORMWITHALPHA.
pub(crate) fn read_color_transform(
    reader: &mut TagReader<'_>,
    alpha: bool,
) -> Result<ColorTransform, GfxError> {
    reader.align();
    let has_add = reader.flag()?;
    let has_multiply = reader.flag()?;
    let bits = reader.ub(4)?;
    let channels = if alpha { 4 } else { 3 };
    let mut transform = ColorTransform::IDENTITY;
    if has_multiply {
        for factor in transform.multiply.iter_mut().take(channels) {
            *factor = f32::from(i16::try_from(reader.sb(bits)?).unwrap_or_default()) / 256.0;
        }
    }
    if has_add {
        for addend in transform.add.iter_mut().take(channels) {
            *addend = f32::from(i16::try_from(reader.sb(bits)?).unwrap_or_default());
        }
    }
    reader.align();
    Ok(transform)
}

/// The kind of a filter of a placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilterKind {
    /// A drop shadow.
    DropShadow,
    /// A blur.
    Blur,
    /// A glow.
    Glow,
    /// A bevel.
    Bevel,
    /// A glow with a gradient.
    GradientGlow,
    /// A convolution matrix.
    Convolution,
    /// A colour matrix.
    ColorMatrix,
    /// A bevel with a gradient.
    GradientBevel,
}

/// A filter of a placement: its kind and its parameters as the tag stores them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    /// The kind.
    pub kind: FilterKind,
    /// The parameters after the type byte, not decoded.
    pub data: Vec<u8>,
}

/// Reads a FILTERLIST.
fn read_filters(reader: &mut TagReader<'_>) -> Result<Vec<Filter>, GfxError> {
    let count = usize::from(reader.u8()?);
    let mut filters = Vec::with_capacity(count);
    for _ in 0..count {
        let code = reader.u8()?;
        let (kind, len) = match code {
            0 => (FilterKind::DropShadow, 23),
            1 => (FilterKind::Blur, 9),
            2 => (FilterKind::Glow, 15),
            3 => (FilterKind::Bevel, 27),
            4 => (FilterKind::GradientGlow, 0),
            5 => (FilterKind::Convolution, 0),
            6 => (FilterKind::ColorMatrix, 80),
            7 => (FilterKind::GradientBevel, 0),
            kind => return Err(GfxError::Filter { kind }),
        };
        let mut data = Vec::new();
        let len = match kind {
            FilterKind::GradientGlow | FilterKind::GradientBevel => {
                let colours = reader.u8()?;
                data.push(colours);
                usize::from(colours) * 5 + 19
            }
            FilterKind::Convolution => {
                let columns = reader.u8()?;
                let rows = reader.u8()?;
                data.extend_from_slice(&[columns, rows]);
                8 + usize::from(columns) * usize::from(rows) * 4 + 5
            }
            _ => len,
        };
        data.extend_from_slice(reader.bytes(len)?);
        filters.push(Filter { kind, data });
    }
    Ok(filters)
}

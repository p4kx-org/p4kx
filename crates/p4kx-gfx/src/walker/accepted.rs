// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::Walker;
use crate::GfxError;
use crate::asset_origins::AcceptedTag;
use crate::assets::{OtherDefinition, UnknownTag};
use crate::bitmap::{read_external, read_jpeg, read_lossless, read_sub_image};
use crate::font::{FontFormat, read_compacted_font, read_font1, read_font2};
use crate::font_info::{read_font_info, read_font_name, read_font4};
use crate::import::{read_import, read_scaling_grid, read_symbol_classes};
use crate::morph::read_morph_shape;
use crate::shape::read_shape;
use crate::tag::{Tag, tag_name};
use crate::text::{read_edit_text, read_static_text};
use crate::timeline::{FrameLabel, Timeline, placed_id, read_exporter_info, read_exports};

const OTHER_DEFINITIONS: [u16; 3] = [7, 34, 60];

impl<'a> Walker<'a, '_> {
    #[expect(clippy::too_many_lines, reason = "one arm per admitted tag type")]
    pub(super) fn read_tag(
        &mut self,
        tag: &Tag<'a>,
        timeline: &mut Timeline,
    ) -> Result<AcceptedTag, GfxError> {
        use AcceptedTag::{
            Bitmap, Exporter, Exports, Font, Frame, Import, Label, Morph, Other, Placement,
            ScalingGrid, Shape, SymbolClasses, Unknown, Unread,
        };
        let assets = &mut self.assets;
        Ok(match tag.code {
            1 => {
                timeline.frames += 1;
                Frame
            }
            4 | 26 | 70 => {
                let id = placed_id(tag)?;
                timeline.placed.extend(id);
                Placement(id)
            }
            43 => {
                timeline.labels.push(FrameLabel {
                    frame: timeline.frames,
                    name: tag.reader().c_string()?.into_owned(),
                });
                Label
            }
            57 | 71 => {
                assets.imports.push(read_import(tag, &mut self.imported)?);
                Import
            }
            76 => {
                let symbols = read_symbol_classes(tag)?;
                let count = symbols.len();
                assets.symbol_classes.extend(symbols);
                SymbolClasses(count)
            }
            78 => {
                assets.scaling_grids.push(read_scaling_grid(tag)?);
                ScalingGrid
            }
            2 | 22 | 32 | 83 => {
                let version = match tag.code {
                    2 => 1,
                    22 => 2,
                    32 => 3,
                    _ => 4,
                };
                assets
                    .shapes
                    .push(read_shape(tag, version, &mut self.budget)?);
                Shape
            }
            46 | 84 => {
                let version = if tag.code == 46 { 1 } else { 2 };
                assets
                    .morph_shapes
                    .push(read_morph_shape(tag, version, &mut self.budget)?);
                Morph
            }
            6 | 21 | 35 | 90 => {
                let version = match tag.code {
                    6 => 1,
                    21 => 2,
                    35 => 3,
                    _ => 4,
                };
                assets.bitmaps.push(read_jpeg(tag, version)?);
                Bitmap
            }
            20 | 36 => {
                assets.bitmaps.push(read_lossless(tag, tag.code == 36)?);
                Bitmap
            }
            1001 | 1009 => {
                let version = if tag.code == 1001 { 1 } else { 2 };
                assets.bitmaps.push(read_external(tag, version)?);
                Bitmap
            }
            1008 => {
                assets.bitmaps.push(read_sub_image(tag)?);
                Bitmap
            }
            10 | 48 | 75 | 1005 | 91 => {
                let font = match tag.code {
                    10 => read_font1(tag, &mut self.budget),
                    48 => read_font2(tag, FontFormat::DefineFont2, &mut self.budget),
                    75 => read_font2(tag, FontFormat::DefineFont3, &mut self.budget),
                    1005 => read_compacted_font(tag),
                    _ => read_font4(tag),
                }?;
                assets.fonts.push(font);
                Font
            }
            13 | 62 => {
                self.font_infos.push((read_font_info(tag)?, tag.site()));
                Other
            }
            88 => {
                self.font_names.push((read_font_name(tag)?, tag.site()));
                Other
            }
            56 => {
                let start = assets.exports.len();
                assets.exports.extend(read_exports(tag)?);
                Exports(start)
            }
            1000 => {
                assets.exporter_info = Some(read_exporter_info(tag)?);
                Exporter
            }
            code if OTHER_DEFINITIONS.contains(&code) => {
                assets.other_definitions.push(OtherDefinition {
                    id: tag.reader().u16()?,
                    code,
                });
                *self.unread.entry(code).or_default() += 1;
                Unread
            }
            code if tag_name(code).is_none() => {
                assets.unknown_tags.push(UnknownTag {
                    code,
                    offset: tag.offset,
                    body: tag.body,
                });
                Unknown
            }
            8 => {
                self.assets.jpeg_tables.get_or_insert(tag.body);
                AcceptedTag::JpegTables
            }
            0 | 5 | 9 | 11 | 28 | 33 | 37 => self.read_other(tag)?,
            code => {
                *self.unread.entry(code).or_default() += 1;
                Unread
            }
        })
    }

    fn read_other(&mut self, tag: &Tag<'a>) -> Result<AcceptedTag, GfxError> {
        let assets = &mut self.assets;
        Ok(match tag.code {
            9 => {
                assets.background.get_or_insert(tag.reader().rgb()?);
                AcceptedTag::Background
            }
            11 | 33 => {
                assets.static_texts.push(read_static_text(
                    tag,
                    tag.code == 33,
                    &mut self.text_budget,
                )?);
                AcceptedTag::StaticText
            }
            37 => {
                assets.edit_texts.push(read_edit_text(tag)?);
                AcceptedTag::EditText
            }
            _ => AcceptedTag::Other,
        })
    }
}

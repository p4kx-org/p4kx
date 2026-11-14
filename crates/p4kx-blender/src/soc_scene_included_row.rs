// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::{SourceValue, SourcedValue};
use p4kx_model::{IncludedObjectKind, IncludedObjectRow, IncludedPlacement};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_included::{ByteRange, IncludedOrigin};

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct ObjectRow {
    ordinal: usize,
    range: ByteRange,
    data: SourcedValue<Object>,
}

impl ObjectRow {
    pub(super) fn read(
        row: IncludedObjectRow<'_>,
        origin: &IncludedOrigin<'_, '_>,
        model_count: usize,
        palette_count: usize,
    ) -> Result<Self, SocSceneError> {
        let range = row.range();
        let (data, complete) = match row.kind() {
            IncludedObjectKind::Placement(record) => {
                let prefix_f64 = prefix_f64(row, &range, origin)?;
                let placement =
                    Placement::read(record, origin, model_count, palette_count, prefix_f64)?;
                (Object::Placement(Box::new(placement)), true)
            }
            IncludedObjectKind::Kind16 => (Object::Kind16, true),
            IncludedObjectKind::Truncated { kind } => (Object::Truncated { kind }, false),
            IncludedObjectKind::UnmeasuredKind { kind } => (Object::UnmeasuredKind { kind }, false),
            IncludedObjectKind::UnmeasuredLayout => (Object::UnmeasuredLayout, false),
            _ => return Err(SocSceneError::Position),
        };
        Ok(Self {
            ordinal: row.ordinal(),
            range: ByteRange::new(range.clone()),
            data: origin.cell(range, "objectRow", SourceValue::Present(data), complete)?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Object {
    Placement(Box<Placement>),
    Kind16,
    Truncated { kind: Option<u32> },
    UnmeasuredKind { kind: u32 },
    UnmeasuredLayout,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Placement {
    model_index: SourcedValue<u16>,
    model_slot: Option<usize>,
    palette_selector: SourcedValue<u8>,
    palette_slot: Option<usize>,
    transform: SourcedValue<[[Option<f64>; 4]; 3]>,
    prefix_f64: SourcedValue<[[Option<f64>; 3]; 2]>,
}

impl Placement {
    fn read(
        record: IncludedPlacement<'_>,
        origin: &IncludedOrigin<'_, '_>,
        model_count: usize,
        palette_count: usize,
        prefix_f64: SourcedValue<[[Option<f64>; 3]; 2]>,
    ) -> Result<Self, SocSceneError> {
        let model = record.model_index();
        let palette = record.palette_byte();
        let rows = record.transform_words().map(|row| {
            row.map(|word| {
                let value = f64::from_bits(word);
                value.is_finite().then_some(value)
            })
        });
        let model_slot = usize::from(model);
        let palette_slot = usize::from(palette);
        Ok(Self {
            model_index: origin.cell(
                record.model_index_range(),
                "modelIndex",
                SourceValue::Present(model),
                true,
            )?,
            model_slot: (model_slot < model_count).then_some(model_slot),
            palette_selector: origin.cell(
                record.palette_range(),
                "paletteSelector",
                SourceValue::Present(palette),
                true,
            )?,
            palette_slot: (palette_slot < palette_count).then_some(palette_slot),
            prefix_f64,
            transform: origin.cell(
                record.transform_range(),
                "transform",
                SourceValue::Present(rows),
                true,
            )?,
        })
    }
}

fn prefix_f64(
    row: IncludedObjectRow<'_>,
    range: &Range<u64>,
    origin: &IncludedOrigin<'_, '_>,
) -> Result<SourcedValue<[[Option<f64>; 3]; 2]>, SocSceneError> {
    const START: usize = size_of::<u32>();
    const LENGTH: usize = size_of::<[[f64; 3]; 2]>();
    let bytes = row
        .bytes()
        .get(START..START + LENGTH)
        .ok_or(SocSceneError::Position)?;
    let mut values = [[None; 3]; 2];
    for (value, bytes) in values.iter_mut().flatten().zip(bytes.as_chunks().0) {
        let decoded = f64::from_le_bytes(*bytes);
        *value = decoded.is_finite().then_some(decoded);
    }
    let start = range.start + START as u64;
    origin.cell(
        start..start + LENGTH as u64,
        "prefixF64",
        SourceValue::Present(values),
        true,
    )
}

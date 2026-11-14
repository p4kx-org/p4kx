// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use base64::engine::general_purpose::STANDARD;
use p4kx_extraction::{Archive, Entry, SourceEncryption};
use serde_json::{Value, json};

use super::Check;
use super::original::{self, NAME_BYTES, Original};

pub(super) fn compare(
    value: &Value,
    table: &Value,
    container: &Archive,
    parent: &Entry,
    entry: &Entry,
    original: &Original<'_>,
) -> Check<()> {
    let mut wire = physical_source(value, &table["source"], entry, parent)?;
    wire.slot = original.ordinal;
    let origin = container
        .entry_origin(entry)
        .ok_or("original member encryption origin")?;
    wire.marker = match origin.encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    compare_table(table, original, &wire)?;
    assert!(
        jsonschema::validator_for(&p4kx_blender::soc_scene_schema())?.is_valid(value),
        "public SOC schema validates the installed projection"
    );

    Ok(())
}

struct Wire<'a> {
    uses: &'a [Value],
    source: &'a Value,
    marker: Option<bool>,
    slot: usize,
}

impl Wire<'_> {
    fn origin(&self, cell: &Value, field: &str, range: Range<u64>) -> Check<()> {
        assert_eq!(cell["sourceSetComplete"], true, "complete datum origin");
        assert_eq!(
            cell["containsEncryptedSource"],
            json!(self.marker),
            "effective member/parent encryption"
        );
        let [reference] = array(cell, "sourceUsesRefs")? else {
            return Err("an original value must identify one physical use".into());
        };
        let used = self
            .uses
            .iter()
            .find(|row| row["id"] == *reference)
            .ok_or("field use missing from source inventory")?;
        assert_eq!(&used["source"], self.source, "same physical member");
        assert_eq!(used["role"], "payload", "original source use role");
        assert_eq!(used["scope"], "payload", "original source byte domain");
        assert_eq!(
            *field_at(used, "/location/namespace")?,
            "socIncludedObjects",
            "coordinate namespace"
        );
        assert_eq!(
            *field_at(used, "/location/field")?,
            field,
            "exact coordinate field"
        );
        assert_eq!(
            *field_at(used, "/location/indices")?,
            json!([
                self.slot.to_string(),
                range.start.to_string(),
                range.end.to_string()
            ]),
            "physical chunk and half-open member byte ranges"
        );
        Ok(())
    }
}

fn field_at<'a>(parent: &'a Value, pointer: &str) -> Check<&'a Value> {
    parent
        .pointer(pointer)
        .ok_or_else(|| "missing wire field".into())
}

pub(super) fn array<'a>(parent: &'a Value, field: &str) -> Check<&'a [Value]> {
    parent
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "wire array missing".into())
}

fn relative_range(range: Range<usize>) -> Value {
    json!({"start":range.start.to_string(),"end":range.end.to_string()})
}

const fn absolute(original: &Original<'_>, range: Range<usize>) -> Range<u64> {
    let base = original.header.offset();
    base + range.start as u64..base + range.end as u64
}

fn physical_source<'a>(
    scene: &'a Value,
    source_id: &Value,
    entry: &Entry,
    parent: &Entry,
) -> Check<Wire<'a>> {
    let inventory = &scene["sourceInventory"];
    let sources = array(inventory, "sources")?;
    let leaf = sources
        .iter()
        .find(|source| source["id"] == *source_id)
        .ok_or("original member source")?;
    assert_eq!(leaf["entryIndex"], entry.index(), "physical member ordinal");
    assert!(
        field_at(leaf, "/path/value")?.as_str() == Some(entry.path()),
        "original member path retained"
    );
    let [ancestor_id] = array(leaf, "ancestors")? else {
        return Err("expected exactly one containing physical Entry".into());
    };
    let ancestor = sources
        .iter()
        .find(|source| source["id"] == *ancestor_id)
        .ok_or("original parent source")?;
    assert_eq!(
        ancestor["entryIndex"],
        parent.index(),
        "exact physical parent Entry"
    );
    Ok(Wire {
        uses: array(inventory, "uses")?,
        source: &leaf["id"],
        marker: None,
        slot: 0,
    })
}

fn compare_paths(original: &Original<'_>, projected: &Value, wire: &Wire<'_>) -> Check<()> {
    for ((name, field), range) in [
        ("models", "modelPath"),
        ("materials", "materialPath"),
        ("palettes", "palettePath"),
    ]
    .into_iter()
    .zip(&original.name_ranges)
    {
        let source = original
            .bytes
            .get(range.clone())
            .ok_or("original path table range")?;
        let rows = array(projected, name)?;
        assert_eq!(
            rows.len(),
            source.len() / NAME_BYTES,
            "original ordered path slots"
        );
        for (ordinal, (bytes, row)) in source
            .as_chunks::<NAME_BYTES>()
            .0
            .iter()
            .zip(rows)
            .enumerate()
        {
            let start = range.start + ordinal * NAME_BYTES;
            let range = start..start + NAME_BYTES;
            assert_eq!(row["ordinal"], ordinal, "physical path ordinal");
            assert_eq!(
                row["range"],
                relative_range(range.clone()),
                "original path byte range"
            );
            if let Some(end) = bytes.iter().position(|&byte| byte == 0) {
                let raw = bytes.get(..end).ok_or("path field prefix")?;
                if let Ok(path) = std::str::from_utf8(raw) {
                    assert!(
                        field_at(row, "/path/value")?.as_str() == Some(path),
                        "original verbatim path bytes"
                    );
                }
            }
            wire.origin(&row["path"], field, absolute(original, range))?;
        }
    }
    Ok(())
}

fn compare_matrix<const COLUMNS: usize, const ROWS: usize>(
    projected: &Value,
    bytes: &[u8],
) -> Check<()> {
    let rows = projected.as_array().ok_or("wire numeric rows")?;
    assert_eq!(rows.len(), ROWS, "original numeric row count");
    for (row, projected) in rows.iter().enumerate() {
        let columns = projected.as_array().ok_or("wire numeric columns")?;
        assert_eq!(columns.len(), COLUMNS, "original numeric column count");
        for (column, projected) in columns.iter().enumerate() {
            let observed = f64::from_le_bytes(original::word(
                bytes,
                (row * COLUMNS + column) * size_of::<f64>(),
            )?);
            if observed.is_finite() {
                assert!(
                    projected.as_f64().map(f64::to_bits) == Some(observed.to_bits()),
                    "original finite f64 bits"
                );
            } else {
                assert!(
                    projected.is_null(),
                    "nonfinite f64 retains its raw word without invalid JSON"
                );
            }
        }
    }
    Ok(())
}

fn compare_row(
    original: &Original<'_>,
    projected: &Value,
    wire: &Wire<'_>,
    range: Range<usize>,
) -> Check<()> {
    assert_eq!(
        projected["range"],
        relative_range(range.clone()),
        "original placement interval"
    );
    wire.origin(
        &projected["data"],
        "objectRow",
        absolute(original, range.clone()),
    )?;
    let row = field_at(projected, "/data/value")?;
    assert_eq!(row["type"], "placement", "original kind-1 row");
    let bytes = original
        .bytes
        .get(range.clone())
        .ok_or("original placement bytes")?;
    let model = u16::from_le_bytes(original::word(bytes, 60)?);
    let [palette] = original::word(bytes, 172)?;
    let model_slot =
        (usize::from(model) < original.name_ranges[0].len() / NAME_BYTES).then_some(model);
    let palette_slot =
        (usize::from(palette) < original.name_ranges[2].len() / NAME_BYTES).then_some(palette);
    assert_eq!(
        *field_at(row, "/modelIndex/value")?,
        model,
        "original model selector"
    );
    assert_eq!(
        row["modelSlot"],
        json!(model_slot),
        "model resolves in its own original table"
    );
    assert_eq!(
        *field_at(row, "/paletteSelector/value")?,
        palette,
        "original palette byte"
    );
    assert_eq!(
        row["paletteSlot"],
        json!(palette_slot),
        "palette resolves in its own original table"
    );
    for (field, bytes) in [
        ("modelIndex", 60..62),
        ("paletteSelector", 172..173),
        ("prefixF64", 4..52),
        ("transform", 64..160),
    ] {
        let range = range.start + bytes.start..range.start + bytes.end;
        wire.origin(&row[field], field, absolute(original, range))?;
    }
    compare_matrix::<3, 2>(
        field_at(row, "/prefixF64/value")?,
        bytes.get(4..52).ok_or("prefix f64 source")?,
    )?;
    compare_matrix::<4, 3>(
        field_at(row, "/transform/value")?,
        bytes.get(64..160).ok_or("transform f64 source")?,
    )
}

fn compare_table(table: &Value, original: &Original<'_>, wire: &Wire<'_>) -> Check<()> {
    use base64::Engine as _;

    assert_eq!(table["ordinal"], original.ordinal, "physical chunk ordinal");
    assert_eq!(
        table["version"],
        original.header.version(),
        "original chunk version"
    );
    assert_eq!(table["byteOrder"], "little", "original chunk byte order");
    assert_eq!(
        table["byteOffset"],
        original.header.offset().to_string(),
        "original SOC member offset"
    );
    assert_eq!(
        table["byteLength"],
        original.header.size().to_string(),
        "original SOC chunk byte length"
    );
    let encoded = field_at(table, "/raw/value")?
        .as_str()
        .ok_or("raw chunk wire string")?;
    assert!(
        STANDARD.decode(encoded)? == original.bytes,
        "complete exact original raw chunk"
    );
    let chunk_span = absolute(original, 0..original.bytes.len());
    wire.origin(&table["raw"], "raw", chunk_span.clone())?;
    wire.origin(&table["tables"], "tables", chunk_span)?;
    compare_tables(field_at(table, "/tables/value")?, original, wire)
}

fn compare_tables(tables: &Value, original: &Original<'_>, wire: &Wire<'_>) -> Check<()> {
    assert_eq!(
        *field_at(tables, "/layout/value")?,
        original.layout,
        "original layout word"
    );
    assert_eq!(
        tables["objectRange"],
        relative_range(original.object_range.clone()),
        "original table bounds"
    );
    assert_eq!(
        *field_at(tables, "/declaredObjectBytes/value")?,
        original.object_range.len(),
        "original table byte count"
    );
    assert_eq!(
        tables["trailingRange"],
        relative_range(original.object_range.end..original.bytes.len()),
        "original bytes beyond declared table"
    );
    let layout_at = original.name_ranges[2].end;
    let object_at = original.object_range.start;
    wire.origin(
        &tables["layout"],
        "layout",
        absolute(original, layout_at..layout_at + 4),
    )?;
    wire.origin(
        &tables["declaredObjectBytes"],
        "declaredObjectBytes",
        absolute(original, object_at - 4..object_at),
    )?;
    compare_paths(original, tables, wire)?;
    let rows = array(tables, "rows")?;
    for (ordinal, range) in original.placement_rows()? {
        let row = rows
            .get(ordinal)
            .ok_or("original placement missing from output")?;
        assert_eq!(row["ordinal"], ordinal, "original placement ordinal");
        compare_row(original, row, wire, range)?;
    }
    Ok(())
}

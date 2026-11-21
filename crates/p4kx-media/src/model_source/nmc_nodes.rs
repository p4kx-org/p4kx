// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::model::{NmcFrame, NmcFrameField, NmcNode, NmcPropertyLine, NmcText};

use super::super::nmc_record::{self as wire, Assignment, NmcRecord, Node, Property, Text};
use super::super::origins;
use super::{Projection, missing};
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter, Status, Subresource};

pub(super) fn emit(
    coordinate: &Projection<'_, '_>,
    node: &NmcNode<'_>,
    bytes: &[u8],
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let frame = node.frame();
    let layout = coordinate.selector(4..28, "nmcNodeLayout", writer)?;
    let mut prepared = coordinate.prepared(layout.clone(), writer)?;
    frame_fields(coordinate, &mut prepared, &frame, writer)?;
    text_fields(
        coordinate,
        &mut prepared,
        node.name(),
        node.properties(),
        &layout,
        writer,
    )?;
    let payload = NmcRecord::Node(Node {
        stored_id: frame.stored_id(),
        parent_word: frame.parent_word(),
        geometry_type: frame.geometry_type(),
        helper: frame.is_helper(),
        local_matrix_bits: frame.local_matrix_bits(),
        companion_matrix_bits: frame.companion_matrix_bits(),
        scale_bits: frame.scale_bits(),
        name: node.name().map(|name| Text {
            utf8: std::str::from_utf8(name.bytes()).ok(),
            display_name_utf8: std::str::from_utf8(name.name_line()).ok(),
            range: coordinate.range(name.range()),
            terminated: name.terminated(),
        }),
        property_entry: node.properties().map(|p| coordinate.range(p.range())),
        property_entry_terminated: node.properties().map(NmcText::terminated),
    });
    prepared.write(
        writer,
        &Subresource::new(
            wire::NODE,
            vec![coordinate.index as u64, frame.ordinal() as u64],
        ),
        Status::Complete,
        &payload,
        &[],
    )?;
    if let Some(properties) = node.properties() {
        for (line, property) in properties.property_lines().enumerate() {
            writer.checkpoint()?;
            write_property(
                coordinate,
                [frame.ordinal(), line],
                &property,
                bytes,
                writer,
            )?;
        }
    }
    Ok(())
}

fn frame_fields(
    coordinate: &Projection<'_, '_>,
    prepared: &mut PreparedRecordOrigins,
    frame: &NmcFrame<'_>,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    for (field, word) in [
        ("stored_id", NmcFrameField::StoredId),
        ("parent_word", NmcFrameField::Parent),
        ("geometry_type", NmcFrameField::GeometryType),
        ("helper", NmcFrameField::GeometryType),
    ] {
        coordinate.bind(prepared, field, frame.range(word), writer)?;
    }
    for (field, words) in [
        ("local_matrix_bits", NmcFrameField::LocalMatrix),
        ("companion_matrix_bits", NmcFrameField::CompanionMatrix),
    ] {
        let start = frame.range(words).start;
        for (row, column) in (0..3).flat_map(|row| (0..4).map(move |column| (row, column))) {
            let word = start + (row * 4 + column) * 4;
            coordinate.bind(
                prepared,
                &format!("{field}/{row}/{column}"),
                word..word + 4,
                writer,
            )?;
        }
    }
    let scale = frame.range(NmcFrameField::Scale);
    for axis in 0..3 {
        let word = scale.start + axis * 4;
        coordinate.bind(
            prepared,
            &format!("scale_bits/{axis}"),
            word..word + 4,
            writer,
        )?;
    }
    Ok(())
}

fn text_fields(
    coordinate: &Projection<'_, '_>,
    prepared: &mut PreparedRecordOrigins,
    name: Option<NmcText<'_>>,
    property: Option<NmcText<'_>>,
    layout: &OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    if let Some(name) = name {
        let text = name.range();
        let slot = slot_range(name);
        let display_end = text.start + name.name_line().len() as u64;
        let display_stop = display_end + u64::from(display_end < slot.end);
        for (field, span) in [
            ("name/utf8", text.clone()),
            ("name/display_name_utf8", text.start..display_stop),
            ("name/range", slot.clone()),
            ("name/terminated", slot),
        ] {
            bind_text(coordinate, prepared, layout, field, span, writer)?;
        }
    } else {
        origins::at(prepared, "/data/payload/name", layout.clone(), writer)?;
    }
    for field in ["property_entry", "property_entry_terminated"] {
        if let Some(property) = property {
            bind_text(
                coordinate,
                prepared,
                layout,
                field,
                slot_range(property),
                writer,
            )?;
        } else {
            origins::at(
                prepared,
                &format!("/data/payload/{field}"),
                layout.clone(),
                writer,
            )?;
        }
    }
    Ok(())
}

const fn slot_range(text: NmcText<'_>) -> Range<u64> {
    let at = text.range();
    at.start..at.end + text.terminated() as u64
}

fn bind_text(
    coordinate: &Projection<'_, '_>,
    prepared: &mut PreparedRecordOrigins,
    layout: &OriginContributions,
    field: &str,
    span: Range<u64>,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut joined = writer.capture_origins();
    joined.include_original(layout)?;
    joined.include_original(&coordinate.origins.payload(
        coordinate.absolute(span),
        field,
        SourceUseRole::Payload,
        writer,
    )?)?;
    origins::at(
        prepared,
        &format!("/data/payload/{field}"),
        joined.finish(),
        writer,
    )
}

fn write_property(
    coordinate: &Projection<'_, '_>,
    [node, ordinal]: [usize; 2],
    line: &NmcPropertyLine<'_>,
    bytes: &[u8],
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let layout = coordinate.selector(line.range(), "nmcPropertyLine", writer)?;
    let mut prepared = coordinate.prepared(layout.clone(), writer)?;
    coordinate.bind(&mut prepared, "range", line.range(), writer)?;
    coordinate.bind(&mut prepared, "raw_utf8", line.range(), writer)?;
    let assignment = if let Some((key, value)) = line.property() {
        let wire = Assignment {
            key: text(bytes, &key)?,
            value: text(bytes, &value)?,
        };
        coordinate.bind(&mut prepared, "assignment/key", key, writer)?;
        bind_text(
            coordinate,
            &mut prepared,
            &layout,
            "assignment/value",
            value,
            writer,
        )?;
        Some(wire)
    } else {
        coordinate.bind(&mut prepared, "assignment", line.range(), writer)?;
        None
    };
    let payload = NmcRecord::NodeProperty(Property {
        raw_utf8: std::str::from_utf8(line.bytes()).ok(),
        range: coordinate.range(line.range()),
        assignment,
    });
    prepared.write(
        writer,
        &Subresource::new(
            wire::PROPERTY,
            vec![coordinate.index as u64, node as u64, ordinal as u64],
        ),
        Status::Complete,
        &payload,
        &[],
    )
}

fn text<'a>(bytes: &'a [u8], span: &Range<u64>) -> Result<Option<&'a str>, MediaError> {
    let start = usize::try_from(span.start).map_err(|_| missing())?;
    let end = usize::try_from(span.end).map_err(|_| missing())?;
    let bytes = bytes.get(start..end).ok_or_else(missing)?;
    Ok(std::str::from_utf8(bytes).ok())
}

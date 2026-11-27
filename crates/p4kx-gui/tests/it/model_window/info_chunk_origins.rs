// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{Event, Modifiers, MouseWheelSource, MouseWheelUnit, Pos2, TouchPhase, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_extraction::{FieldOrigin, InputScope, SourceInventory, SourceLocation, SourceUseRole};
use p4kx_gui::{Language, Model, ModelPreview, Unavailable, ViewerSupport};
use p4kx_testkit::{TestEntry, TestModel};
use serde_json::Value;

use crate::viewing::{Site, TestResult, model_preview, opened_at, select};

const MAIN: &str = "Data/Objects/skin.skin";
const GEOMETRY: &str = "Data/Objects/skin_lod1.skinm";
const MAIN_TYPE: u32 = 0xDEAD_BEEF;
const GEOMETRY_TYPE: u32 = 0xCAFE_BABE;
const SCROLL_STEPS: usize = 6;

#[test]
fn visible_chunk_markers_follow_the_clear_model_and_encrypted_lod_geometry() -> TestResult {
    let level = TestModel::two_bone_skin().with_geometry_chunk(GEOMETRY_TYPE, [10, 11, 12]);
    let model = TestModel::two_bone_skin()
        .with_chunk(MAIN_TYPE, [7, 8, 9])
        .with_lod(level);
    let main = model.cgf();
    let geometry = model.lod_cgfm(1).ok_or("LOD geometry fixture")?;
    let site = Site {
        entries: vec![
            TestEntry::zstd(MAIN, main.clone()),
            TestEntry::zstd("Data/Objects/skin.skinm", model.cgfm()),
            TestEntry::zstd(
                "Data/Objects/skin_lod1.skin",
                model.lod_cgf(1).ok_or("LOD model fixture")?,
            ),
            TestEntry::zstd(GEOMETRY, geometry.clone()).encrypted(),
        ],
        installation: false,
        key: true,
    };
    let (_folder, mut harness) = opened_at(
        Language::BritishEnglish,
        ViewerSupport::Unavailable(Unavailable::NoDevice),
        site,
        |builder| builder,
    )?;
    select(&mut harness, "skin.skin")?;
    let preview = model_preview(harness.state()).ok_or("model preview")?;
    assert_chunk_provenance(preview, &main, &geometry)?;

    scroll_to_chunks(&mut harness)?;
    assert_marker(
        &mut harness,
        &format!("0x{MAIN_TYPE:08X}"),
        "Unencrypted source data",
    )?;
    assert_marker(
        &mut harness,
        &format!("0x{GEOMETRY_TYPE:08X}"),
        "Includes encrypted source data",
    )?;
    Ok(())
}

fn assert_chunk_provenance(preview: &ModelPreview, main: &[u8], geometry: &[u8]) -> TestResult {
    let info = preview.info().ok_or("model facts")?;
    let origins = preview
        .info_provenance()
        .ok_or("chunk provenance")?
        .map_err(ToString::to_string)?;
    assert_eq!(
        origins.unread_chunks().len(),
        info.unread.len(),
        "visible notes only"
    );
    assert_eq!(
        origins.chunk_total().contains_encrypted_source(),
        Some(true),
        "the admitted geometry contributes to the total"
    );

    for (type_id, path, bytes, encrypted) in [
        (MAIN_TYPE, MAIN, main, false),
        (GEOMETRY_TYPE, GEOMETRY, geometry, true),
    ] {
        let formatted = format!("0x{type_id:08X}");
        let position = info
            .unread
            .iter()
            .position(|note| note.type_id == formatted)
            .ok_or("visible chunk type")?;
        let row = origins
            .unread_chunks()
            .get(position)
            .ok_or("visible chunk origin")?;
        let field = &row.type_id;
        assert_eq!(
            field.contains_encrypted_source(),
            Some(encrypted),
            "the physical type word"
        );
        assert_type_word(field, origins.inventory(), path, bytes, type_id)?;
    }
    Ok(())
}

fn scroll_to_chunks(harness: &mut Harness<'_, Model>) -> TestResult {
    harness.get_by_label("Info").click();
    harness.run_steps(4);
    let nodes = visible_point(harness, "Nodes")?;
    harness.event(Event::PointerMoved(nodes));
    harness.run_steps(2);
    for _ in 0..SCROLL_STEPS {
        scroll_info(harness);
    }
    Ok(())
}

pub(super) fn reveal_info_row(harness: &mut Harness<'_, Model>, label: &str) -> TestResult {
    let panel = p4kx_gui::ui::regions(&harness.ctx)
        .preview
        .ok_or("visible model preview")?;
    harness.event(Event::PointerMoved(panel.center()));
    harness.run_steps(2);
    for _ in 0..SCROLL_STEPS {
        if visible_point(harness, label).is_ok() {
            return Ok(());
        }
        scroll_info(harness);
    }
    visible_point(harness, label).map(|_| ())
}

fn scroll_info(harness: &mut Harness<'_, Model>) {
    harness.event(Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: vec2(0.0, -4.0),
        modifiers: Modifiers::NONE,
        phase: TouchPhase::Move,
        source: MouseWheelSource::Wheel,
    });
    harness.run_steps(2);
}

pub(super) fn assert_marker(
    harness: &mut Harness<'_, Model>,
    label: &str,
    marker: &str,
) -> TestResult {
    harness.ctx.style_mut_of(harness.ctx.theme(), |style| {
        style.interaction.tooltip_delay = 0.0;
        style.interaction.show_tooltips_only_when_still = false;
    });
    harness.event(Event::PointerGone);
    harness.run_steps(2);
    let point = visible_point(harness, label)?;
    harness.event(Event::PointerMoved(point));
    harness.run_steps(5);
    assert!(
        harness.query_by_label(marker).is_some(),
        "field {label}: {marker}"
    );
    Ok(())
}

fn visible_point(
    harness: &Harness<'_, Model>,
    label: &str,
) -> Result<Pos2, Box<dyn std::error::Error>> {
    let panel = p4kx_gui::ui::regions(&harness.ctx)
        .preview
        .ok_or("visible model preview")?;
    let points: Vec<_> = harness
        .query_all_by_label(label)
        .map(|cell| cell.rect().center())
        .collect();
    points
        .iter()
        .copied()
        .find(|point| panel.contains(*point))
        .ok_or_else(|| format!("visible {label} in {panel:?}, found {points:?}").into())
}

fn assert_type_word(
    field: &FieldOrigin,
    inventory: &SourceInventory,
    path: &str,
    bytes: &[u8],
    expected: u32,
) -> TestResult {
    assert!(field.source_set_complete(), "complete original type word");
    let [reference] = field.source_uses_refs() else {
        return Err("expected one original type word".into());
    };
    let used = inventory
        .iter_uses()
        .find(|used| used.id() == reference)
        .ok_or("bound type use")?;
    assert_eq!(used.scope(), InputScope::Payload, "the decoded data domain");
    assert_eq!(
        used.role(),
        SourceUseRole::Payload,
        "the emitted original ID"
    );
    let SourceLocation::Subresource {
        namespace,
        indices,
        field,
    } = used.location()
    else {
        return Err("expected a typed original byte interval".into());
    };
    assert_eq!(namespace, "model-byte-range", "original model coordinates");
    assert_eq!(field, "typeId", "original numeric chunk field");
    let [start, end] = indices.as_slice() else {
        return Err("expected the type-word byte range".into());
    };
    assert_eq!(
        bytes.get(usize::try_from(*start)?..usize::try_from(*end)?),
        Some(expected.to_le_bytes().as_slice()),
        "actual type word in the original fixture"
    );
    let source = inventory
        .iter_sources()
        .find(|source| source.id() == used.source())
        .ok_or("physical origin")?;
    assert_eq!(
        serde_json::to_value(source)?
            .pointer("/path/value")
            .and_then(Value::as_str),
        Some(path),
        "the exact source Entry"
    );
    Ok(())
}

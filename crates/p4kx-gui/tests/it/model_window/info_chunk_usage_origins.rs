// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui_kittest::kittest::Queryable;
use p4kx_extraction::chunks::ivo;
use p4kx_extraction::{FieldOrigin, InputScope, SourceInventory, SourceLocation, SourceUseRole};
use p4kx_gui::{Language, MAX_LISTED_CHUNKS, ModelInfoProvenance, Unavailable, ViewerSupport};
use p4kx_testkit::{TestEntry, TestModel};
use serde_json::Value;

use super::info_chunk_origins::{hover_info_value, reveal_info_row};
use super::info_mesh_origins::{provenance, synthetic_site};
use crate::viewing::{TestResult, model_preview, opened_at, select};

const CLEAR_UNKNOWN: u32 = 0xDEAD_BEEF;
const ENCRYPTED_UNKNOWN: u32 = 0xCAFE_BABE;
const ENCRYPTED_GEOMETRY: &str = "Data/Objects/usage_lod1.cgfm";

#[test]
fn chunk_usage_counts_bind_original_words_beyond_the_visible_notes() -> TestResult {
    let primary = (0..MAX_LISTED_CHUNKS).fold(TestModel::cube(), |model, _| {
        model.with_chunk(CLEAR_UNKNOWN, [0; 8])
    });
    let level = TestModel::cube().with_geometry_chunk(ENCRYPTED_UNKNOWN, [0; 8]);
    let model = primary.with_lod(level);
    let geometry = model.lod_cgfm(1).ok_or("LOD geometry fixture")?;
    let entries = vec![
        TestEntry::zstd("Data/Objects/usage.cgf", model.cgf()),
        TestEntry::zstd("Data/Objects/usage.cgfm", model.cgfm()),
        TestEntry::zstd(ENCRYPTED_GEOMETRY, geometry.clone()).encrypted(),
    ];
    let (_folder, mut harness) = opened_at(
        Language::BritishEnglish,
        ViewerSupport::Unavailable(Unavailable::NoDevice),
        synthetic_site(entries),
        |builder| builder,
    )?;
    select(&mut harness, "usage.cgf")?;
    let preview = model_preview(harness.state()).ok_or("model preview")?;
    let info = preview.info().ok_or("model facts")?;
    assert_eq!(info.unread.len(), MAX_LISTED_CHUNKS, "bounded unread notes");
    assert!(
        info.unread.iter().all(|row| row.type_id != "0xCAFEBABE"),
        "the encrypted unknown type is beyond the visible notes"
    );
    assert_eq!(
        info.chunks.unknown,
        MAX_LISTED_CHUNKS + 1,
        "the count covers all unknown rows"
    );
    assert_usage_origins(provenance(preview)?, &geometry)?;

    let label = format!(
        "{} ({} read, {} skipped, {} unknown)",
        info.chunks.total, info.chunks.read, info.chunks.skipped, info.chunks.unknown
    );
    harness.get_by_label("Info").click();
    harness.run_steps(4);
    reveal_info_row(&mut harness, &label)?;
    hover_info_value(&mut harness, &label)?;
    assert_eq!(
        harness
            .query_all_by_label("Includes encrypted source data")
            .count(),
        4,
        "total and each usage count have their own evidence"
    );
    for label in [
        "Total chunks",
        "Read chunks",
        "Skipped known chunks",
        "Unknown chunks",
    ] {
        assert!(
            harness.query_by_label(label).is_some(),
            "tooltip line {label}"
        );
    }
    Ok(())
}

fn assert_usage_origins(proof: &ModelInfoProvenance, geometry: &[u8]) -> TestResult {
    let groups = proof.chunk_usage();
    for (origin, expected) in [
        (&groups.read, ivo::SKIN_2.id()),
        (&groups.skipped, ivo::TERMINATOR.id()),
        (&groups.unknown, ENCRYPTED_UNKNOWN),
    ] {
        assert!(!origin.source_set_complete(), "partial classifier evidence");
        assert_eq!(
            origin.contains_encrypted_source(),
            Some(true),
            "encrypted contributing file"
        );
        assert_geometry_type_word(proof.inventory(), origin, geometry, expected)?;
    }
    assert_eq!(
        groups.unknown.source_uses_refs().len(),
        4,
        "two words per owning file"
    );
    Ok(())
}

#[test]
fn an_empty_chunk_usage_group_keeps_unknown_evidence() -> TestResult {
    let model = TestModel::cube();
    let entries = vec![
        TestEntry::zstd("Data/Objects/known.cgf", model.cgf()).encrypted(),
        TestEntry::zstd("Data/Objects/known.cgfm", model.cgfm()),
    ];
    let (_folder, mut harness) = opened_at(
        Language::BritishEnglish,
        ViewerSupport::Unavailable(Unavailable::NoDevice),
        synthetic_site(entries),
        |builder| builder,
    )?;
    select(&mut harness, "known.cgf")?;
    let preview = model_preview(harness.state()).ok_or("model preview")?;
    let info = preview.info().ok_or("model facts")?;
    let proof = provenance(preview)?;
    assert_eq!(
        info.chunks.unknown, 0,
        "the IVO cube contains only known chunk types"
    );
    assert_eq!(
        proof.chunk_total().contains_encrypted_source(),
        Some(true),
        "encrypted total"
    );
    let empty = &proof.chunk_usage().unknown;
    assert!(
        empty.source_uses_refs().is_empty(),
        "no positive classification witness"
    );
    assert!(
        !empty.source_set_complete(),
        "absence of a witness is incomplete"
    );
    assert_eq!(
        empty.contains_encrypted_source(),
        None,
        "no inheritance from the total"
    );
    Ok(())
}

fn assert_geometry_type_word(
    inventory: &SourceInventory,
    origin: &FieldOrigin,
    geometry: &[u8],
    expected: u32,
) -> TestResult {
    for reference in origin.source_uses_refs() {
        let site = inventory
            .iter_uses()
            .find(|site| site.id() == reference)
            .ok_or("classification use")?;
        let SourceLocation::Subresource {
            namespace,
            field,
            indices,
        } = site.location()
        else {
            continue;
        };
        if namespace != "model-byte-range" || field != "typeId" {
            continue;
        }
        let file = inventory
            .iter_sources()
            .find(|file| file.id() == site.source())
            .ok_or("owning file")?;
        if serde_json::to_value(file)?
            .pointer("/path/value")
            .and_then(Value::as_str)
            != Some(ENCRYPTED_GEOMETRY)
        {
            continue;
        }
        assert_eq!(site.scope(), InputScope::Payload, "owning file payload");
        assert_eq!(
            site.role(),
            SourceUseRole::Selector,
            "original classification input"
        );
        let [start, end] = indices.as_slice() else {
            return Err("expected one original type word range".into());
        };
        assert_eq!(
            geometry.get(usize::try_from(*start)?..usize::try_from(*end)?),
            Some(expected.to_le_bytes().as_slice()),
            "actual type bytes of the admitted geometry companion"
        );
        return Ok(());
    }
    Err("missing original encrypted classification input".into())
}

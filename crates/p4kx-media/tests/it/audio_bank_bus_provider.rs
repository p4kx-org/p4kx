// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::hirc_select;
use p4kx_extraction::{Archive, bank::Bank};
use p4kx_media::Status;
use p4kx_testkit::{ArchiveBuilder, TestEntry, TestHircObject, test_key};
use serde_json::json;

use super::audio_bank_container_node_provider::fixture::{bank, export_observed, layout};
use super::audio_bank_state_provider::row;
use super::support::audio_provider::TestResult;

const SUMMARY: &str = "audio-bank-hirc-bus";
const DUCK: &str = "audio-bank-hirc-bus-duck";
const UNKNOWN_VOLUME: u32 = 0x7fc1_2345;

#[test]
fn bus_source_fields_and_media_rows_keep_original_spans_and_encryption() -> TestResult {
    let body = fixture(0);
    let object = bank(154, TestHircObject::raw(8, 55, &body[..]));
    for encrypted in [false, true] {
        export_observed(&object, encrypted, false, |rows, evidence| {
            let summary = row(rows, SUMMARY, &[1, 0])?;
            layout(summary.1, "decoded", Status::Complete);
            evidence.word(
                summary,
                "observed/device_shareset_id",
                "busDeviceSharesetId",
                0,
            )?;
            evidence.word(summary, "observed/duck_count", "busDuckCount", 1)?;
            let duck = row(rows, DUCK, &[1, 0, 0])?;
            evidence.word(duck, "bus_id", "busDuckTargetId", 0)?;
            evidence.word(duck, "volume_bits", "busDuckVolumeBits", UNKNOWN_VOLUME)?;
            evidence.byte(duck, "fade_curve", "busDuckFadeCurve", 0xfe)?;
            assert_eq!(
                duck.1.data.pointer("/payload/fade_out_time/value"),
                Some(&json!(-8)),
                "original signed fade duration"
            );
            row(rows, "audio-bank-hirc-node-property", &[1, 0, 0, 0])?;
            row(rows, "audio-bank-hirc-node-fx-slot", &[1, 0, 1, 0])?;
            row(rows, "audio-bank-hirc-inline-state", &[1, 0])?;
            row(rows, "audio-bank-hirc-bus-rtpc", &[1, 0])?;
            Ok(())
        })?;
        object_view(&object, encrypted)?;
    }
    Ok(())
}

fn object_view(bytes: &[u8], encrypted: bool) -> TestResult {
    const PATH: &str = "bus-fixture.bnk";
    let folder = tempfile::tempdir()?;
    let archive_path = folder.path().join("Data.p4k");
    let entry = TestEntry::stored(PATH, bytes);
    let entry = if encrypted { entry.encrypted() } else { entry };
    ArchiveBuilder::new().entry(entry).write_to(&archive_path)?;
    let archive = Archive::open_with_key(&archive_path, Some(test_key()))?;
    let bank = Bank::from_archive_observed(&archive, PATH)?;
    let view = hirc_select::select_with_sources(&bank, None, None, 0, 1);
    let view = serde_json::to_value(view.shown.first().ok_or("one physical ObjectView")?)?;
    assert_eq!(
        view.pointer("/node_fields/object_status/value"),
        Some(&json!("decoded")),
        "CLI and MCP source view shares the v154 observation"
    );
    let target = view
        .pointer("/node_fields/bus/duck_targets/0/volume_bits")
        .ok_or("exact volume field")?;
    assert_eq!(
        target.get("value"),
        Some(&json!(UNKNOWN_VOLUME)),
        "raw binary32 survives"
    );
    assert_eq!(
        target.get("containsEncryptedSource"),
        Some(&json!(encrypted)),
        "the owning original BNK is the value's source"
    );
    assert_eq!(
        target.get("sourceSetComplete"),
        Some(&json!(true)),
        "the observed volume bytes are a complete source set"
    );
    let inventory = view
        .pointer("/node_fields/source_inventory/uses")
        .and_then(serde_json::Value::as_array)
        .ok_or("exact references inventory")?;
    let references = target
        .get("sourceUsesRefs")
        .and_then(serde_json::Value::as_array)
        .ok_or("direct per-value source use")?;
    assert_eq!(
        references.len(),
        1,
        "only the original volume word contributes"
    );
    for id in references {
        let source = inventory
            .iter()
            .find(|row| row.get("id") == Some(id))
            .ok_or("closed original value provenance")?;
        assert_eq!(
            source.pointer("/location/field"),
            Some(&json!("busDuckVolumeBits")),
            "the exact observed word is cited"
        );
    }
    Ok(())
}

#[test]
fn child_and_unsupported_bus_exports_report_separate_original_dispositions() -> TestResult {
    let body = fixture(25);
    let child = bank(154, TestHircObject::raw(18, 55, &body[..]));
    export_observed(&child, false, false, |rows, evidence| {
        let summary = row(rows, SUMMARY, &[1, 0])?;
        layout(summary.1, "decoded", Status::Complete);
        evidence.word(summary, "observed/override_bus_id", "busOverrideId", 25)?;
        assert!(
            summary
                .1
                .data
                .pointer("/payload/observed/device_shareset_id")
                .is_none(),
            "a child carries no device word"
        );
        Ok(())
    })?;
    let raw = bank(153, TestHircObject::raw(18, 55, body));
    export_observed(&raw, false, false, |rows, _| {
        layout(
            row(rows, SUMMARY, &[1, 0])?.1,
            "unsupported",
            Status::Complete,
        );
        assert!(
            rows.iter().all(|row| row.subresource.namespace != DUCK),
            "an unsupported layout has no guessed duck rows"
        );
        Ok(())
    })
}

fn fixture(parent: u32) -> Vec<u8> {
    let mut bytes = parent.to_le_bytes().to_vec();
    if parent == 0 {
        bytes.extend(0_u32.to_le_bytes());
    }
    bytes.extend([1, 0xed]);
    bytes.extend(0xff80_0000_u32.to_le_bytes());
    bytes.extend([0, 0]);
    bytes.extend(0_u32.to_le_bytes());
    bytes.extend([0x80, 0, 0, 2, 0, 0, 0, 0x80]);
    bytes.extend(7_i32.to_le_bytes());
    bytes.extend(
        [0_u32, 1, 0, UNKNOWN_VOLUME]
            .into_iter()
            .flat_map(u32::to_le_bytes),
    );
    bytes.extend([-8_i32, 9].into_iter().flat_map(i32::to_le_bytes));
    bytes.extend([0xfe, 0xfa, 0, 1, 2]);
    bytes.extend(31_u32.to_le_bytes());
    bytes.extend([0x82, 0, 0, 0, 0]);
    bytes
}

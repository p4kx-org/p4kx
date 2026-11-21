// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::hirc_select::select_with_sources;
use p4kx_extraction::hirc_view::ObjectView;
use p4kx_extraction::{Bank, HircKind, HircLayout};
use p4kx_media::{MediaRecord, Status};
use serde_json::{Value, json};

use super::audio_bank_container_node_provider::fixture::{Evidence, bank, export, layout};
use super::audio_bank_state_provider::row;
use super::support::audio_provider::TestResult;

#[path = "audio_bank_effect_metadata_fixture.rs"]
mod fixture;

use self::fixture::{BANK_DATA_ID, NONFINITE, OPAQUE, PLUGIN_ID};

const PLUGIN: &str = "audio-bank-hirc-plugin";
const BANK_DATA: &str = "audio-bank-hirc-plugin-bank-data";
const OVERRIDE: &str = "audio-bank-hirc-plugin-state-override";
const PLUGIN_RTPC: &str = "audio-bank-hirc-plugin-rtpc-point";
const MODULATOR: &str = "audio-bank-hirc-modulator";
const PROPERTY: &str = "audio-bank-hirc-modulator-property";
const MODULATOR_RTPC: &str = "audio-bank-hirc-modulator-rtpc-point";
const INLINE_STATE: &str = "audio-bank-hirc-inline-state-property";
const FX: &str = "audio-bank-hirc-node-fx";
const FX_SLOT: &str = "audio-bank-hirc-node-fx-slot";

#[test]
fn effects_preserve_opaque_bytes_repeated_references_and_original_mapping_words() -> TestResult {
    for (kind, slug) in [(16, "fx-share-set"), (17, "fx-custom")] {
        let bytes = fixture::fixture(154, kind, &fixture::effect()?);
        export(&bytes, true, HircLayout::Decoded, |rows, evidence| {
            let summary = row(rows, PLUGIN, &[1, 0])?;
            layout(summary.1, "decoded", Status::Complete);
            assert_eq!(
                summary
                    .1
                    .data
                    .pointer("/payload/observed/coverage/object_kind"),
                Some(&json!(slug)),
                "share set and custom effect keep independent original kind codes"
            );
            evidence.word(summary, "observed/plugin_id", "pluginId", PLUGIN_ID)?;
            evidence.word(
                summary,
                "observed/parameters/declared_size",
                "pluginParametersSize",
                u32::try_from(OPAQUE.len())?,
            )?;
            evidence.bytes(
                summary,
                "observed/parameters/range",
                "pluginParametersBytes",
                &OPAQUE,
            )?;
            for ordinal in 0..2 {
                let mapping = row(rows, BANK_DATA, &[1, 0, ordinal])?;
                evidence.byte(mapping, "index", "pluginBankDataIndex", 0)?;
                evidence.word(mapping, "source_id", "pluginBankDataSourceId", BANK_DATA_ID)?;
            }
            check_controls(rows, evidence)
        })?;
    }
    Ok(())
}

fn check_controls(rows: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    let first_override = row(rows, OVERRIDE, &[1, 0, 0])?;
    evidence.byte(
        first_override,
        "accumulation",
        "pluginOverrideAccumulation",
        0xF9,
    )?;
    evidence.word(
        first_override,
        "value_bits",
        "pluginOverrideValueBits",
        NONFINITE,
    )?;
    let second_override = row(rows, OVERRIDE, &[1, 0, 1])?;
    evidence.word(second_override, "value_bits", "pluginOverrideValueBits", 0)?;
    assert_eq!(
        second_override.1.data.pointer("/payload/property_id/value"),
        Some(&json!(132)),
        "the repeated original variable-length ID remains present"
    );
    let point = row(rows, PLUGIN_RTPC, &[1, 0, 0, 0])?;
    evidence.word(point, "from_bits", "rtpcFromBits", NONFINITE)?;
    let authored_state = row(rows, INLINE_STATE, &[1, 0, 0, 0, 0])?;
    assert_eq!(
        authored_state.1.data.pointer("/payload/value_bits/value"),
        Some(&json!(NONFINITE)),
        "the shared inline State reader preserves the literal property word"
    );
    Ok(())
}

#[test]
fn device_uses_effect_base_then_owned_slots_and_preserves_empty_forms() -> TestResult {
    let bytes = fixture::fixture(154, 21, &fixture::device()?);
    export(&bytes, true, HircLayout::Decoded, |rows, evidence| {
        check_controls(rows, evidence)?;
        check_device_slots(rows, evidence)
    })?;
    let mut empty = fixture::empty_effect()?;
    empty.push(0);
    let bytes = fixture::fixture(154, 21, &empty);
    export(&bytes, false, HircLayout::Decoded, |rows, evidence| {
        let summary = row(rows, PLUGIN, &[1, 0])?;
        evidence.bytes(
            summary,
            "observed/parameters/range",
            "pluginParametersBytes",
            &[],
        )?;
        assert_eq!(
            summary
                .1
                .data
                .pointer("/payload/observed/parameters/complete"),
            Some(&json!(true)),
            "zero authored bytes form a complete payload"
        );
        let slots = row(rows, FX, &[1, 0, 0])?;
        assert!(
            slots
                .1
                .data
                .pointer("/payload/observed/bypass_all_bits")
                .is_none(),
            "an empty device list has no bypass byte"
        );
        evidence.byte(slots, "observed/declared_count", "nodeFxCount", 0)
    })
}

fn check_device_slots(rows: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    let slots = row(rows, FX, &[1, 0, 0])?;
    assert!(
        slots
            .1
            .data
            .pointer("/payload/observed/override_parent_bits")
            .is_none(),
        "a device owns its slot list"
    );
    evidence.byte(
        slots,
        "observed/bypass_all_bits",
        "nodeFxBypassAllBits",
        0xC1,
    )?;
    let first = row(rows, FX_SLOT, &[1, 0, 0, 0])?;
    let second = row(rows, FX_SLOT, &[1, 0, 0, 1])?;
    assert_eq!(
        first.1.data.pointer("/payload/slot_index/value"),
        Some(&json!(4)),
        "the original index stays visible for each device slot"
    );
    assert_eq!(
        second.1.data.pointer("/payload/slot_index/value"),
        Some(&json!(4)),
        "the original index stays visible for each device slot"
    );
    assert_eq!(
        first.1.data.pointer("/payload/effect_id/value"),
        Some(&json!(0)),
        "the explicit zero device reference stays present"
    );
    assert_eq!(
        second.1.data.pointer("/payload/slot_bits/value"),
        Some(&json!(0x80)),
        "unknown device slot bits remain literal"
    );
    Ok(())
}

#[test]
fn each_modulator_kind_keeps_planar_scalar_range_and_control_words() -> TestResult {
    for (kind, slug) in [(19, "lfo"), (20, "envelope"), (22, "time-modulator")] {
        let bytes = fixture::fixture(154, kind, &fixture::modulator());
        export(&bytes, true, HircLayout::Decoded, |rows, evidence| {
            let summary = row(rows, MODULATOR, &[1, 0])?;
            layout(summary.1, "decoded", Status::Complete);
            assert_eq!(
                summary
                    .1
                    .data
                    .pointer("/payload/observed/coverage/object_kind"),
                Some(&json!(slug)),
                "the original modulator kind is preserved"
            );
            let first = row(rows, PROPERTY, &[1, 0, 0, 0])?;
            evidence.byte(first, "property_id", "modulatorPropertyId", 0xFF)?;
            evidence.word(first, "words/scalar_bits", "modulatorPropertyScalarBits", 3)?;
            assert!(
                first.1.data.pointer("/payload/interpretation").is_none(),
                "modulator words keep their authored meaning"
            );
            let ranged = row(rows, PROPERTY, &[1, 0, 1, 0])?;
            evidence.word(
                ranged,
                "words/minimum_bits",
                "modulatorPropertyMinimumBits",
                0x8000_0000,
            )?;
            evidence.word(
                ranged,
                "words/maximum_bits",
                "modulatorPropertyMaximumBits",
                NONFINITE,
            )?;
            evidence.word(
                row(rows, MODULATOR_RTPC, &[1, 0, 0, 0])?,
                "from_bits",
                "rtpcFromBits",
                NONFINITE,
            )
        })?;
    }
    Ok(())
}

#[test]
fn malformed_prefixes_leave_original_body_and_following_object_available() -> TestResult {
    let mut device = fixture::device()?;
    device.pop();
    let mut trailing = fixture::effect()?;
    trailing.push(0xAB);
    let mut modulator = fixture::modulator();
    modulator.pop();
    for (kind, namespace, body) in [
        (16, PLUGIN, fixture::truncated_blob()),
        (17, PLUGIN, fixture::truncated_bank_data()?),
        (17, PLUGIN, fixture::malformed_override()?),
        (17, PLUGIN, trailing),
        (21, PLUGIN, device),
        (22, MODULATOR, modulator),
    ] {
        let bytes = fixture::fixture(154, kind, &body);
        export(&bytes, false, HircLayout::Malformed, |rows, evidence| {
            let summary = row(rows, namespace, &[1, 0])?;
            layout(summary.1, "malformed", Status::Partial);
            if kind == 16 {
                evidence.word(
                    summary,
                    "observed/parameters/declared_size",
                    "pluginParametersSize",
                    u32::MAX,
                )?;
                evidence.bytes(
                    summary,
                    "observed/parameters/range",
                    "pluginParametersBytes",
                    &OPAQUE,
                )?;
                assert_eq!(
                    summary
                        .1
                        .data
                        .pointer("/payload/observed/parameters/complete"),
                    Some(&json!(false)),
                    "the bounded opaque prefix is incomplete"
                );
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn unknown_bank_versions_have_explicit_raw_dispositions() -> TestResult {
    for version in [153, 155] {
        for (kind, namespace) in [
            (16, PLUGIN),
            (17, PLUGIN),
            (19, MODULATOR),
            (20, MODULATOR),
            (21, PLUGIN),
            (22, MODULATOR),
        ] {
            let bytes = fixture::fixture(version, kind, &[0x9F, 0x21]);
            export(&bytes, false, HircLayout::Unsupported, |rows, _| {
                let summary = row(rows, namespace, &[1, 0])?;
                layout(summary.1, "unsupported", Status::Complete);
                assert!(
                    summary.1.data.pointer("/payload/observed").is_none(),
                    "unsupported layout has an explicit coverage record"
                );
                Ok(())
            })?;
        }
    }
    Ok(())
}

#[test]
fn object_view_shared_by_cli_and_mcp_exposes_exact_source_values_and_schema() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("original.bnk");
    let schema = serde_json::to_value(schemars::schema_for!(ObjectView<'_>))?;
    let validator = jsonschema::validator_for(&schema)?;
    for (code, kind, body, path_to_value) in [
        (
            21,
            HircKind::AudioDevice,
            fixture::device()?,
            "/source_fields/observed/parameters/declared_size",
        ),
        (
            19,
            HircKind::Lfo,
            fixture::modulator(),
            "/source_fields/observed/properties/rows/0/property_id",
        ),
    ] {
        std::fs::write(&path, fixture::fixture(154, code, &body))?;
        let bank = Bank::from_file_observed(&path)?;
        let page = select_with_sources(&bank, Some(kind), None, 0, 1);
        let object = page.shown.first().ok_or("original typed occurrence")?;
        let json = serde_json::to_value(object)?;
        validator
            .validate(&json)
            .map_err(|error| error.to_string())?;
        let cell = json
            .pointer(path_to_value)
            .ok_or("original sourced value")?;
        assert_eq!(
            cell["sourceSetComplete"],
            json!(true),
            "the file source completes the field receipt"
        );
        assert_eq!(
            cell["containsEncryptedSource"],
            Value::Null,
            "the loose-file Archive lineage is unknown"
        );
        let references = cell
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("exact field source references")?;
        assert!(
            !references.is_empty(),
            "a decoded source value names its consulted bytes"
        );
    }
    Ok(())
}

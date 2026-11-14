// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use p4kx_extraction::ConflictPolicy;
use p4kx_extraction::model::MaterialSourceLimits;
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};
use serde_json::Value;

use crate::material_observer_support::{self as support, Probe, TestResult};
use crate::{ExportOutcome, export_with};

#[test]
fn material_observer_cancellation_stops_at_the_actual_event_and_preserves_complete_receipts()
-> TestResult {
    cancellation_case(5, 2, false)
}

#[test]
fn material_observer_cancellation_after_an_optional_capture_cut_stops_at_the_actual_event()
-> TestResult {
    cancellation_case(2048, 1024, true)
}

fn cancellation_case(layers: usize, stop_at: usize, cut: bool) -> TestResult {
    let previous = support::PreviousGeneration::new()?;
    let mut material = String::from("<Material><MatLayers>");
    for _ in 0..layers {
        material.push_str("<Layer PaletteTint='bad'/>");
    }
    material.push_str("</MatLayers></Material>");
    let next = support::archive(previous.folder.path(), "next.p4k", material.as_bytes())?;
    let mut options = support::options();
    options.conflict = ConflictPolicy::Overwrite;
    let mut limits = MaterialSourceLimits::default();
    if cut {
        options.limits.record_bytes = 16 << 10;
        limits.text_bytes = 1;
    }
    let probe = Arc::new(Probe::new(options.cancel.clone(), Some(stop_at)));
    let observed = support::observed(Arc::clone(&probe), limits)?;
    let error = export_with(&next, &previous.output, &options, &observed)
        .err()
        .ok_or("cancelled observation published")?;
    assert!(
        support::is_cancelled(&error),
        "actual observer cancellation stays fatal cancellation: {error}"
    );
    previous.assert_unchanged()?;
    assert_eq!(
        probe.palette_events.load(Ordering::SeqCst),
        stop_at,
        "cancellation at one actual PaletteTint event stops before the next event, including after an optional capture cut"
    );
    Ok(())
}
#[test]
fn material_observer_noop_preserves_actual_text_and_binary_payload_and_companion_bytes()
-> TestResult {
    let text = b"<Material Name='same'><MatLayers><Layer PaletteTint='2'/></MatLayers></Material>"
        .to_vec();
    let binary = TestElement::new("Material")
        .attribute("Name", "same")
        .child(
            TestElement::new("MatLayers")
                .child(TestElement::new("Layer").attribute("PaletteTint", "2")),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Big);
    for material in [text, binary] {
        noop_case(&material)?;
    }
    Ok(())
}

fn noop_case(material: &[u8]) -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path(), "Data.p4k", material)?;
    let options = support::options();
    let baseline_output = folder.path().join("baseline");
    let baseline = support::published(export_with(
        &archive,
        &baseline_output,
        &options,
        &support::standard()?,
    )?)?;
    let probe = Arc::new(Probe::new(options.cancel.clone(), None));
    let output = folder.path().join("observed");
    let observed = support::published(export_with(
        &archive,
        &output,
        &options,
        &support::observed(Arc::clone(&probe), MaterialSourceLimits::default())?,
    )?)?;
    assert!(
        probe.events.load(Ordering::SeqCst) > 0,
        "the parity control actually enters the neutral adapter"
    );
    support::same_outputs(
        &baseline_output.join(&baseline.artifact_base),
        &baseline,
        &output.join(&observed.artifact_base),
        &observed,
    )
}
#[test]
fn material_observer_never_precedes_source_size_or_invalid_configuration_refusal() -> TestResult {
    for source_bytes in [1, MaterialSourceLimits::default().source_bytes] {
        let folder = tempfile::tempdir()?;
        let archive = support::archive(folder.path(), "Data.p4k", b"<Material/>")?;
        let mut limits = MaterialSourceLimits::default();
        limits.source_bytes = source_bytes;
        limits.depth = 65;
        let options = support::options();
        let probe = Arc::new(Probe::new(options.cancel.clone(), Some(1)));
        let output = folder.path().join("media");
        let ExportOutcome::Incomplete { diagnostic, .. } = export_with(
            &archive,
            &output,
            &options,
            &support::observed(Arc::clone(&probe), limits)?,
        )?
        else {
            return Err("expected original diagnosed configuration/source refusal".into());
        };
        let rows = fs::read_to_string(
            diagnostic
                .parent()
                .ok_or("diagnostic directory")?
                .join("material.jsonl"),
        )?;
        let record: Value =
            serde_json::from_str(rows.lines().next().ok_or("actual refusal record")?)?;
        let reason = if source_bytes == 1 {
            "source-bytes"
        } else {
            "invalid-limits"
        };
        assert_eq!(
            record
                .pointer("/data/payload/reason/type")
                .and_then(Value::as_str),
            Some(reason)
        );
        assert_eq!(
            probe.events.load(Ordering::SeqCst),
            0,
            "the neutral observer cannot run before the original guards"
        );
        assert!(!options.cancel.is_cancelled());
    }
    Ok(())
}

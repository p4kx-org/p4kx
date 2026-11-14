// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::io;
use std::io::Read;

use p4kx_assembly::BodyVariant;
use p4kx_blender::{PlayerDocument, PlayerDocumentSink, PlayerSource, PlayerSourceCapture};
use p4kx_extraction::{Archive, CancelToken};
use serde_json::Value;

use super::TestResult;

#[derive(Default)]
struct Documents(Vec<Original>);

struct Original {
    index: usize,
    source: Value,
    ordinal: usize,
    bytes: Vec<u8>,
}

impl PlayerDocumentSink for Documents {
    fn document(&mut self, document: PlayerDocument<'_>) -> io::Result<()> {
        assert_eq!(document.format, "textXml", "fixture originals use text XML");
        assert!(!document.lossy, "fixture originals decode without repair");
        assert!(
            document.origin.source_set_complete(),
            "root receipt has exact source"
        );
        self.0.push(Original {
            index: document.index,
            source: serde_json::to_value(document.source)?,
            ordinal: document.original.entry().ordinal() as usize,
            bytes: document.bytes.to_vec(),
        });
        Ok(())
    }
}

#[test]
fn explicit_bodies_share_original_motion_and_document_identity() -> TestResult {
    let (_folder, archive) = super::fixture_with_entries([])?;
    let cancel = CancelToken::new();
    let mut documents = Documents::default();
    let capture = PlayerSourceCapture::read(
        &archive,
        &[BodyVariant::Female, BodyVariant::Male],
        &cancel,
        &mut documents,
    )?;
    let capture = serde_json::to_value(capture)?;
    verify_bodies(&capture)?;
    verify_documents(&archive, &capture, documents)?;
    Ok(())
}

fn verify_bodies(capture: &Value) -> TestResult {
    let rows = capture
        .get("bodies")
        .and_then(Value::as_array)
        .ok_or("captured bodies")?;
    let [female, male] = rows.as_slice() else {
        return Err("expected both explicit bodies".into());
    };
    let mut sources = Vec::new();
    for (row, code) in [(female, "female"), (male, "male")] {
        assert_eq!(
            row.pointer("/bodyVariant").and_then(Value::as_str),
            Some(code),
            "caller order preserved"
        );
        assert_eq!(
            row.pointer("/body/value/type").and_then(Value::as_str),
            Some("present"),
            "both original explicit body sources were read"
        );
        sources.push(
            row.pointer("/body/value/source")
                .ok_or("physical body source")?,
        );
    }
    let [female, male] = sources.as_slice() else {
        return Err("expected two sourced body rows".into());
    };
    assert_ne!(
        female, male,
        "physical origins identify separate CHF Entries"
    );
    let registered = capture
        .pointer("/sourceInventory/sources")
        .and_then(Value::as_array)
        .ok_or("physical inventory")?;
    assert!(
        sources
            .iter()
            .all(|source| registered.iter().any(|row| row.get("id") == Some(source))),
        "both CHF identities resolve in the same inventory",
    );
    let run = capture.pointer("/motion/components/value/0/databaseGraph/branches");
    assert_eq!(
        run.and_then(Value::as_array).map(Vec::len),
        Some(2),
        "shared graph has one physical root and branch"
    );
    Ok(())
}

fn verify_documents(archive: &Archive, capture: &Value, documents: Documents) -> TestResult {
    let registry = capture
        .pointer("/sourceInventory/sources")
        .and_then(Value::as_array)
        .ok_or("physical inventory")?;
    let mut actual = BTreeMap::new();
    assert_eq!(
        documents.0.len(),
        5,
        "one raw document for each loadout/controller/ADB/definition physical source"
    );
    for original in documents.0 {
        let entry = archive
            .entries()
            .get(original.ordinal)
            .ok_or("physical archive ordinal")?;
        let mut expected = Vec::new();
        archive
            .open_entry_verified(entry)?
            .read_to_end(&mut expected)?;
        assert_eq!(
            original.bytes, expected,
            "sink receives exact verified original bytes"
        );
        assert!(
            registry
                .iter()
                .any(|record| record.get("id") == Some(&original.source)),
            "sink evidence uses the shared physical inventory"
        );
        assert!(
            actual.insert(original.index, original.source).is_none(),
            "duplicate alias references reuse one XML document"
        );
    }
    let component = capture
        .pointer("/motion/components/value/0")
        .ok_or("one shared motion component")?;
    for path in [
        "/controller/value/data",
        "/thirdPersonDatabase/value/data",
        "/authoredOptions/sharedFragmentFile/value/data",
    ] {
        let index = component
            .pointer(path)
            .and_then(Value::as_u64)
            .ok_or("document ref")?;
        assert!(
            actual.contains_key(&usize::try_from(index)?),
            "unresolved document {path}"
        );
    }
    Ok(())
}

#[test]
fn a_single_shared_capture_preserves_existing_payload_and_exact_origin_ids() -> TestResult {
    let (_folder, archive) = super::fixture_with_entries([])?;
    let cancel = CancelToken::new();
    let existing = serde_json::to_value(PlayerSource::read(&archive, BodyVariant::Male, &cancel)?)?;
    let mut documents = Documents::default();
    let shared = serde_json::to_value(PlayerSourceCapture::read(
        &archive,
        &[BodyVariant::Male],
        &cancel,
        &mut documents,
    )?)?;
    for field in ["loadouts", "motion"] {
        assert_eq!(
            existing.get(field),
            shared.get(field),
            "{field} keeps its original data and use IDs"
        );
    }
    same_legacy_inventory(&existing, &shared)?;
    assert_eq!(
        existing.pointer("/body"),
        shared.pointer("/bodies/0/body"),
        "a single body still has identical original data and use IDs"
    );
    Ok(())
}

pub(super) fn same_legacy_inventory(existing: &Value, shared: &Value) -> TestResult {
    let existing_sources = existing
        .pointer("/sourceInventory/sources")
        .and_then(Value::as_array)
        .ok_or("v1 physical sources")?;
    let shared_sources = shared
        .pointer("/sourceInventory/sources")
        .and_then(Value::as_array)
        .ok_or("shared physical sources")?;
    assert!(
        existing_sources
            .iter()
            .all(|source| shared_sources.contains(source)),
        "same existing physical source identities"
    );
    let existing_uses = existing
        .pointer("/sourceInventory/uses")
        .and_then(Value::as_array)
        .ok_or("v1 uses")?;
    let new_uses = shared
        .pointer("/sourceInventory/uses")
        .and_then(Value::as_array)
        .ok_or("shared uses")?;
    assert!(
        existing_uses.iter().all(|used| new_uses.contains(used)),
        "all v1 source use rows keep identical references and coordinates"
    );
    Ok(())
}

struct Refuse;

impl PlayerDocumentSink for Refuse {
    fn document(&mut self, _: PlayerDocument<'_>) -> io::Result<()> {
        Err(io::Error::other("test document sink rejected the bytes"))
    }
}

#[test]
fn sink_failures_and_invalid_body_sets_fail_the_capture() -> TestResult {
    let (_folder, archive) = super::fixture_with_entries([])?;
    let cancel = CancelToken::new();
    let duplicate = PlayerSourceCapture::read(
        &archive,
        &[BodyVariant::Male, BodyVariant::Male],
        &cancel,
        &mut Refuse,
    );
    assert!(
        matches!(
            duplicate,
            Err(p4kx_blender::PlayerSourceError::BodySelection)
        ),
        "duplicate body is explicit"
    );
    let refused = PlayerSourceCapture::read(&archive, &[BodyVariant::Male], &cancel, &mut Refuse);
    assert!(
        matches!(refused, Err(p4kx_blender::PlayerSourceError::Xml(_))),
        "sink failure aborts preparation"
    );
    Ok(())
}

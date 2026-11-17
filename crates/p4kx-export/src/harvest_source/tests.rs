// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{
    Archive, Guid, InputScope, SourceEncryptionState, SourceLocation, SourceUseRole, SourceValue,
};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};

use crate::ExportAccountingError;
use crate::catalog_read::original::with_entry_capture;
use crate::export_ledger::SourceDisposition;
use crate::harvest_locations::Soc;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn opened(payload: Vec<u8>) -> Result<(tempfile::TempDir, Archive), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("unrelated.txt", "x"))
        .entry(TestEntry::stored("harvest.socpak", payload))
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}

#[test]
fn an_empty_container_is_a_successful_source_without_an_invented_soc() -> TestResult {
    let (_folder, archive) = opened(TestContainer::new().build())?;
    let (soc, ledger) =
        with_entry_capture(&archive, |capture| super::read(&archive, 1, capture, None))?;
    assert_eq!(soc, Soc::Plain, "empty is a valid source result");
    let [outer] = ledger.entries.as_slice() else {
        return Err("one actual source Entry expected".into());
    };
    assert_eq!(outer.entry_index, Some(1), "actual outer index");
    assert_eq!(
        outer.container_entry_index, None,
        "no invented nested Entry"
    );
    assert_eq!(
        outer.disposition,
        SourceDisposition::Projected,
        "successful zero-row projection"
    );
    Ok(())
}

#[test]
fn a_failed_outer_open_does_not_claim_an_inner_read() -> TestResult {
    let (_folder, archive) = opened(b"invalid container".to_vec())?;
    let (soc, ledger) =
        with_entry_capture(&archive, |capture| super::read(&archive, 1, capture, None))?;
    assert!(matches!(soc, Soc::Unreadable(_)), "the container failed");
    let [outer] = ledger.entries.as_slice() else {
        return Err("one failed source Entry expected".into());
    };
    assert_eq!(
        outer.path.as_str(),
        "harvest.socpak",
        "exact failed identity"
    );
    assert_eq!(
        outer.disposition,
        SourceDisposition::ReadFailed,
        "actual open failure"
    );
    Ok(())
}

#[test]
fn only_the_selected_first_soc_is_observed() -> TestResult {
    let container = TestContainer::new()
        .entry(TestEntry::stored("note.txt", "ignored"))
        .entry(TestEntry::stored("first.soc", "plain source"))
        .entry(TestEntry::zstd("second.soc", "unread source").with_payload(b"bad".to_vec()));
    let (_folder, archive) = opened(container.build())?;
    let (soc, ledger) =
        with_entry_capture(&archive, |capture| super::read(&archive, 1, capture, None))?;
    assert_eq!(soc, Soc::Plain, "a successful scan can find no preset");
    let [outer, inner] = ledger.entries.as_slice() else {
        return Err("outer plus selected SOC expected".into());
    };
    assert_eq!(
        outer.disposition,
        SourceDisposition::Projected,
        "container opened"
    );
    assert_eq!(
        inner.container_entry_index,
        Some(1),
        "actual inner index, not filtered position"
    );
    assert_eq!(
        inner.path.as_str(),
        "harvest.socpak!first.soc",
        "nested identity survives"
    );
    assert_eq!(
        inner.disposition,
        SourceDisposition::Projected,
        "unused corrupt SOC is not read"
    );
    Ok(())
}

#[test]
fn a_corrupt_selected_soc_retains_its_nested_identity() -> TestResult {
    let container = TestContainer::new()
        .entry(TestEntry::zstd("bad.soc", "expected source").with_payload(b"bad".to_vec()));
    let (_folder, archive) = opened(container.build())?;
    let (soc, ledger) =
        with_entry_capture(&archive, |capture| super::read(&archive, 1, capture, None))?;
    assert!(matches!(soc, Soc::Unreadable(_)), "selected source failed");
    let [outer, inner] = ledger.entries.as_slice() else {
        return Err("actual outer and inner operations expected".into());
    };
    assert_eq!(
        outer.disposition,
        SourceDisposition::Projected,
        "directory read succeeded"
    );
    assert_eq!(
        inner.path.as_str(),
        "harvest.socpak!bad.soc",
        "failed inner identity"
    );
    assert_eq!(
        inner.disposition,
        SourceDisposition::ReadFailed,
        "source decoder failed"
    );
    Ok(())
}

const PRESET: &str = "1f712a22-c9d5-438e-8605-fac7651e30fc";

#[test]
fn verified_ranges_are_bound_to_the_selected_inner_entry() -> TestResult {
    let bytes = format!("x\0harvestpreset\0{PRESET}");
    let container = TestContainer::new()
        .entry(TestEntry::stored("decoy.txt", bytes.as_bytes()))
        .entry(TestEntry::stored("first.soc", bytes.as_bytes()))
        .entry(TestEntry::stored("second.soc", bytes.as_bytes()));
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("unrelated.txt", "x"))
        .entry(TestEntry::stored("harvest.socpak", container.build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let preset: Guid = PRESET.parse()?;
    let (read, inner, inventory) = with_entry_capture(&archive, |capture| {
        let (read, ledger) =
            super::read_verified(&archive, 1, capture, None, &HashSet::from([preset]))?;
        let inner = ledger
            .entries
            .get(1)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?
            .original()?
            .payload_reference(capture)?;
        Ok((read, inner, capture.source_uses().freeze()?))
    })?;
    let SourceValue::Present(inner) = inner.value() else {
        return Err("selected inner Entry identity expected".into());
    };
    assert_eq!(
        inner.entry_index(),
        1,
        "uses ordinal among all nested Entries"
    );
    assert_eq!(read.fact, Soc::Preset(preset), "verified first result");
    let selected = read.selected.ok_or("selected original GUID")?;
    assert_eq!(selected.value(), Some(&preset), "same verified token");
    let origin = selected.origin();
    assert!(
        origin.source_set_complete(),
        "both authentic token uses present"
    );
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "inherited outer encryption"
    );
    assert_eq!(origin.source_uses_refs().len(), 2, "key and GUID only");
    for reference in origin.source_uses_refs() {
        let observed = inventory
            .uses()
            .iter()
            .find(|used| used.id() == reference)
            .ok_or("registered token use not found")?;
        assert_eq!(
            observed.source(),
            inner.input_ref(),
            "same physically selected member"
        );
        assert!(
            inventory
                .sources()
                .iter()
                .any(|source| source.id() == observed.source()),
            "token use resolves to the physical inventory"
        );
        assert_eq!(
            observed.scope(),
            InputScope::Payload,
            "decoded member bytes"
        );
        assert_eq!(
            observed.effective_encryption(),
            SourceEncryptionState::Encrypted,
            "payload encryption is inherited from the actual outer container"
        );
        let SourceLocation::Subresource {
            namespace,
            indices,
            field,
        } = observed.location()
        else {
            return Err("original SOC coordinates expected".into());
        };
        assert_eq!(
            namespace, "socHarvestPreset",
            "preserved token coordinate namespace"
        );
        match field.as_str() {
            "key" => {
                assert_eq!(indices.as_slice(), &[2, 15], "whole key");
                assert_eq!(
                    observed.role(),
                    SourceUseRole::Selector,
                    "authored selection"
                );
            }
            "guid" => {
                assert_eq!(
                    indices.as_slice(),
                    &[16, 52],
                    "whole GUID including its final byte"
                );
                assert_eq!(observed.role(), SourceUseRole::Payload, "authored value");
            }
            _ => return Err("unexpected SOC preset evidence field".into()),
        }
    }
    Ok(())
}

#[test]
fn a_crc_failure_beyond_the_first_chunk_never_binds_early_preset_bytes() -> TestResult {
    let mut expected = format!("preset\0{PRESET}\0").into_bytes();
    expected.resize((64 << 10) + 17, b'x');
    let mut written = expected.clone();
    written.push(b'x');
    expected.push(b'y');
    let container = TestContainer::new()
        .entry(TestEntry::stored("selected.soc", expected).with_payload(written));
    let (_folder, archive) = opened(container.build())?;
    let preset: Guid = PRESET.parse()?;
    let (legacy, read, ledger, inventory) = with_entry_capture(&archive, |capture| {
        let (legacy, _) = super::read(&archive, 1, capture, None)?;
        let (read, ledger) =
            super::read_verified(&archive, 1, capture, None, &HashSet::from([preset]))?;
        Ok((legacy, read, ledger, capture.source_uses().freeze()?))
    })?;
    assert_eq!(
        legacy,
        Soc::Preset(preset),
        "legacy remains first-match-only"
    );
    assert!(
        matches!(read.fact, Soc::Unreadable(_)),
        "the verified reader reaches the bad CRC"
    );
    assert!(
        read.selected.is_none(),
        "partial read never binds original value"
    );
    assert_eq!(
        ledger.entries.get(1).map(|entry| entry.disposition),
        Some(SourceDisposition::ReadFailed),
        "failure retains the read operation for the selected inner member"
    );
    assert!(
        inventory.uses().iter().all(|used| !matches!(
            used.location(),
            SourceLocation::Subresource { namespace, .. } if namespace == "socHarvestPreset"
        )),
        "failed read registers no token evidence"
    );
    Ok(())
}

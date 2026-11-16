// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::io;

use p4kx_dataforge::RecordJsonUseRole::{Conversion, Payload, Selector};
use p4kx_dataforge::{
    DataForge, JsonLayout, RecordJson, RecordJsonBytes, RecordJsonObserver, RecordJsonRecordField,
    RecordJsonSource, RecordJsonStep, RecordJsonUse, RecordJsonUseRole,
};

use super::{DANGLING_GUID, STORED_GUID, TestResult, database, first_record, raw};

#[derive(Default)]
struct LinkUses(BTreeMap<String, MemberUses>);

struct MemberUses {
    sources: Vec<RecordJsonUse>,
    complete: bool,
}

impl LinkUses {
    fn observation(&self, path: &str) -> TestResult<&MemberUses> {
        self.0
            .get(path)
            .ok_or_else(|| format!("missing observation for {path}").into())
    }

    fn member(&self, path: &str) -> TestResult<&[RecordJsonUse]> {
        let observed = self.observation(path)?;
        assert!(observed.complete, "original contributors for {path}");
        Ok(&observed.sources)
    }
}

impl RecordJsonObserver for LinkUses {
    fn value(&mut self, _: &str, _: &[RecordJsonSource<'_>], _: bool) -> io::Result<()> {
        Ok(())
    }

    fn reference_value(
        &mut self,
        pointer: &str,
        sources: &[RecordJsonUse],
        complete: bool,
    ) -> io::Result<()> {
        let observed = MemberUses {
            sources: sources.to_vec(),
            complete,
        };
        assert!(
            self.0.insert(pointer.to_owned(), observed).is_none(),
            "each emitted member is observed once: {pointer}"
        );
        Ok(())
    }
}

#[test]
fn dangling_reference_keeps_exact_guid_and_incomplete_error_evidence() -> TestResult {
    let bytes = database();
    let input = DataForge::open(&bytes)?;
    let record = first_record(&input)?;
    let mut capture = LinkUses::default();
    let mut writer = RecordJson::new(JsonLayout::Compact);
    let mut observed = Vec::new();
    writer.write_observed(&record, &mut observed, &mut capture)?;
    let mut plain = Vec::new();
    writer.write(record.tree(), &mut plain)?;
    assert_eq!(observed, plain, "both writers emit the same dangling link");

    let guid = capture.member("/dangling/guid")?;
    let error = capture.observation("/dangling/error")?;
    assert!(!error.complete, "the target lookup was unresolved");
    let slot = RecordJsonBytes::Slot;
    assert_eq!(
        member_bytes(&bytes, guid, slot, Payload)?,
        DANGLING_GUID,
        "the emitted GUID uses only the caller GUID suffix"
    );
    assert_eq!(
        member_bytes(&bytes, &error.sources, slot, Conversion)?,
        DANGLING_GUID,
        "the resolution error uses that same original lookup input"
    );
    for member in [guid, error.sources.as_slice()] {
        assert_eq!(
            member
                .iter()
                .filter(|used| matches!(used.source, RecordJsonSource::Bytes { .. }))
                .count(),
            1,
            "no other bytes supplied either dangling member"
        );
        assert!(
            member
                .iter()
                .all(|used| !matches!(used.source, RecordJsonSource::Record { .. })),
            "no target Record was admitted"
        );
    }
    Ok(())
}

#[test]
fn observed_reference_members_keep_original_payload_and_selector_roles() -> TestResult {
    let bytes = database();
    let input = DataForge::open(&bytes)?;
    let record = first_record(&input)?;
    let mut capture = LinkUses::default();
    let mut writer = RecordJson::new(JsonLayout::Compact);
    let mut observed = Vec::new();
    writer.write_observed(&record, &mut observed, &mut capture)?;
    let mut plain = Vec::new();
    writer.write(record.tree(), &mut plain)?;
    assert_eq!(observed, plain, "observation preserves canonical JSON");
    assert_stored_members(&bytes, &capture)?;
    assert_weak_members(&bytes, &capture)
}

fn assert_stored_members(bytes: &[u8], capture: &LinkUses) -> TestResult {
    let guid = capture.member("/reference/guid")?;
    assert_eq!(
        member_bytes(bytes, guid, RecordJsonBytes::Slot, Payload)?,
        STORED_GUID,
        "the exact caller GUID suffix supplies the emitted value"
    );
    assert!(
        guid.iter()
            .all(|used| !matches!(used.source, RecordJsonSource::Record { .. })),
        "no target table field supplies the authored caller GUID"
    );
    let path = capture.member("/reference/path")?;
    assert_eq!(
        member_bytes(bytes, path, RecordJsonBytes::Slot, Selector)?,
        STORED_GUID,
        "the same suffix selects the target filepath"
    );
    assert_eq!(
        member_bytes(bytes, path, RecordJsonBytes::Text, Payload)?,
        b"selected.xml",
        "the selected duplicate GUID row supplies the original text"
    );
    let pool = capture.member("/references/0/guid")?;
    assert_eq!(
        member_bytes(bytes, pool, RecordJsonBytes::ArrayElement, Payload)?,
        STORED_GUID,
        "a Reference pool element supplies only its GUID suffix"
    );
    assert_eq!(
        member_bytes(bytes, pool, RecordJsonBytes::ArrayHeader, Selector)?.len(),
        size_of::<[u32; 2]>(),
        "the selected pool element keeps its array admission"
    );
    Ok(())
}

fn assert_weak_members(bytes: &[u8], capture: &LinkUses) -> TestResult {
    let guid = capture.member("/local/guid")?;
    assert_eq!(
        member_bytes(bytes, guid, RecordJsonBytes::Slot, Selector)?.len(),
        size_of::<[u32; 2]>(),
        "weak words only select the target"
    );
    assert!(
        guid.iter().any(|used| used.role == Payload
            && used.source
                == RecordJsonSource::Record {
                    index: 0,
                    field: RecordJsonRecordField::Guid,
                }),
        "the owner Record supplies the local weak GUID"
    );
    let name = capture.member("/local/type")?;
    assert!(
        name.iter()
            .any(|used| used.role == Payload
                && used.source == RecordJsonSource::StructName { index: 0 }),
        "the selected instance supplies the Node type"
    );
    let at = capture.member("/local/at")?;
    assert_eq!(
        member_bytes(bytes, at, RecordJsonBytes::Text, Conversion)?,
        b"nodes",
        "the original property name spells the target path"
    );
    assert!(
        at.iter()
            .all(|used| !matches!(used.source, RecordJsonSource::StructName { .. })),
        "type metadata does not spell an instance path"
    );
    let header = RecordJsonBytes::ArrayHeader;
    let route = member_bytes(bytes, at, header, Selector)?;
    for member in [guid, name, capture.member("/local/path")?] {
        assert_eq!(
            member_bytes(bytes, member, header, Selector)?,
            route,
            "all members share the physical ownership route"
        );
    }
    Ok(())
}

#[test]
fn full_and_scoped_reference_members_keep_the_same_caller_route() -> TestResult {
    let bytes = database();
    let input = DataForge::open(&bytes)?;
    let record = first_record(&input)?;
    let mut writer = RecordJson::new(JsonLayout::Compact);
    let mut full = LinkUses::default();
    writer.write_observed(&record, &mut io::sink(), &mut full)?;
    let mut partial = LinkUses::default();
    writer.write_observed_value(
        &record,
        &[RecordJsonStep::Field(6), RecordJsonStep::Field(0)],
        &mut io::sink(),
        &mut partial,
    )?;
    let uses = full.member("/nested/ownedLink/guid")?;
    assert_eq!(
        uses,
        partial.member("/guid")?,
        "the canonical full and selected output expose the same callers"
    );
    assert!(
        uses.iter().any(|used| {
            used.role == Conversion
                && matches!(
                    used.source,
                    RecordJsonSource::PropertyStorage {
                        target_used: true,
                        ..
                    }
                )
        }),
        "the caller's inline class route is preserved as a layout conversion"
    );
    Ok(())
}

fn member_bytes<'archive>(
    bytes: &'archive [u8],
    inputs: &[RecordJsonUse],
    kind: RecordJsonBytes,
    role: RecordJsonUseRole,
) -> TestResult<&'archive [u8]> {
    let source = inputs.iter().find(|used| {
        used.role == role
            && matches!(used.source, RecordJsonSource::Bytes { kind: found, .. } if kind == found)
    });
    raw(bytes, source.map(|used| used.source), kind)
}

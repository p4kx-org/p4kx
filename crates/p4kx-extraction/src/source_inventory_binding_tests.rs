// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ptr;
use std::sync::Arc;

use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde::Serialize;

use super::SourceUses;
use crate::{
    Archive, InputScope, PhysicalSource, PhysicalSourceRef, SourceInput, SourceInventory,
    SourceLocation, SourceUseDescription, SourceUseRole, source_evidence_error,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn many_uses_share_the_physical_reference_and_keep_its_wire_string() -> TestResult {
    let uses = SourceUses::with_compact_coordinates();
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    for record_index in 0..64 {
        scope.at(
            SourceLocation::DataForge {
                record_index,
                field: "/value".into(),
            },
            SourceUseRole::Payload,
        )?;
    }
    let inventory = uses.freeze()?;
    assert_eq!(inventory.source_count(), 1, "one registered input");
    assert_eq!(inventory.use_count(), 64, "all distinct coordinates remain");
    let physical = inventory.iter_sources().next().ok_or("frozen input")?;
    for used in inventory.iter_uses() {
        assert!(
            ptr::eq(used.source().as_str(), physical.id().as_str()),
            "all uses share the physical identity's string allocation"
        );
    }
    assert_eq!(
        serde_json::to_string(physical.id())?,
        "\"u0\"",
        "the physical identity retains the transparent string wire format"
    );
    let schema = schemars::schema_for!(PhysicalSourceRef).to_value();
    assert_eq!(
        schema.get("type").and_then(serde_json::Value::as_str),
        Some("string"),
        "the physical source reference keeps its JSON schema type"
    );
    assert_eq!(
        PhysicalSourceRef::new("u0"),
        *physical.id(),
        "text equality"
    );
    assert!(
        PhysicalSourceRef::new("u10") < PhysicalSourceRef::new("u2"),
        "lexical ordering"
    );
    Ok(())
}

#[test]
fn attaching_original_binding_preserves_frozen_source_and_registered_use_rows() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("binding.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("bound.bin", b"fixture"))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let observed = archive
        .entries()
        .first()
        .and_then(|entry| archive.observed_entry(entry))
        .ok_or("original observed Entry")?;
    let uses = SourceUses::with_compact_coordinates();
    let scope = uses.observe(SourceInput::ArchiveView(observed), InputScope::Payload)?;
    let unbound = {
        let mut registry = source_evidence_error::lock(&uses.registry)?;
        let row = registry
            .sources
            .get_mut(scope.source_ref())
            .ok_or("original physical row")?;
        let binding = Arc::get_mut(row)
            .and_then(|source| source.binding.take())
            .ok_or("previous binding")?;
        drop(registry);
        binding
    };
    assert!(
        unbound.matches(&observed.binding()),
        "test removes only the original binding"
    );
    let original = scope.at(
        SourceLocation::Metadata {
            field: "name".into(),
        },
        SourceUseRole::Selector,
    )?;
    let before = uses.freeze()?;
    let rebound = uses.observe(SourceInput::ArchiveView(observed), InputScope::Payload)?;
    assert_eq!(
        scope.source_ref(),
        rebound.source_ref(),
        "binding uses the same physical identity"
    );
    assert!(
        rebound.matches_entry(&observed)?,
        "registry gains the authoritative binding"
    );
    let after = uses.freeze()?;
    assert_frozen_binding(&before, &after)?;
    assert_frozen_use(&before, &after, original.description())
}

fn assert_frozen_use(
    before: &SourceInventory,
    after: &SourceInventory,
    recorded: &SourceUseDescription,
) -> TestResult {
    let original_row = before
        .iter_uses()
        .find(|row| row.id() == recorded.id())
        .ok_or("original frozen use")?;
    let rebound_row = after
        .iter_uses()
        .find(|row| row.id() == recorded.id())
        .ok_or("rebound frozen use")?;
    assert!(
        ptr::eq(recorded, original_row),
        "freeze retains the original Arc-owned use"
    );
    assert!(
        ptr::eq(original_row, rebound_row),
        "physical binding changes preserve registered use identity"
    );
    Ok(())
}

fn assert_frozen_binding(before: &SourceInventory, after: &SourceInventory) -> TestResult {
    #[derive(Serialize)]
    struct Contiguous<'inventory> {
        sources: &'inventory [PhysicalSource],
        uses: &'inventory [SourceUseDescription],
    }

    let previous = before
        .iter_sources()
        .next()
        .ok_or("initial frozen physical row")?;
    let current = after
        .iter_sources()
        .next()
        .ok_or("later frozen physical row")?;
    assert!(
        previous.binding.is_none(),
        "the frozen row preserves its prior authority"
    );
    assert!(
        current.binding.is_some(),
        "the later snapshot records the attached authority"
    );
    assert!(
        !ptr::eq(previous, current),
        "only the updated physical row is copied"
    );
    assert_eq!(
        before, after,
        "runtime authority is outside document equality"
    );
    assert_eq!(
        serde_json::to_vec(before)?,
        serde_json::to_vec(after)?,
        "wire excludes runtime bindings"
    );

    assert_eq!(
        serde_json::to_vec(after)?,
        serde_json::to_vec(&Contiguous {
            sources: after.sources(),
            uses: after.uses()
        })?,
        "the borrowed inventory writer matches the previous slice wire byte for byte",
    );
    Ok(())
}

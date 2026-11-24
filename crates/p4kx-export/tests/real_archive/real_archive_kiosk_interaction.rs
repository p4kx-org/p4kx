// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_export::catalog_queries::equipment::interactions::{
    InteractionSnapshot, with_interactions_from,
};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, Guid, InputScope,
    RecordTree, SourceEncryption, SourceInput, SourceUses, SourceValueState, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};

use super::{KIOSK, KIOSK_VR_ALIGNMENT, TestResult, original_component};
use crate::real_archive_item_components::cell;
use crate::real_archive_item_durability::{self as proof, Trace};

struct Selection {
    guid: Guid,
    path: String,
    ordinal: u32,
    expected: bool,
}

impl Selection {
    fn read() -> TestResult<Self> {
        Ok(Self {
            guid: env::var("P4KX_KIOSK_OWNER_GUID")?.parse()?,
            path: env::var("P4KX_KIOSK_OWNER_PATH")?,
            ordinal: env::var("P4KX_KIOSK_COMPONENT_ORDINAL")?.parse()?,
            expected: env::var("P4KX_KIOSK_EXPECTED_FLAG")?.parse()?,
        })
    }

    fn index(&self, forge: &DataForge<'_>) -> TestResult<u32> {
        let mut selected = None;
        for record in forge.records().filter(|record| record.guid() == self.guid) {
            if record.record_path()? != self.path {
                continue;
            }
            if selected.replace(record.index()).is_some() {
                return Err("the nominated GUID and path match multiple physical Records".into());
            }
        }
        selected.ok_or_else(|| "the nominated Kiosk Record was not found".into())
    }
}

struct OriginalFlag<'read, 'source> {
    ordinal: u32,
    component: &'read Original<'source>,
    flag: &'read Original<'source>,
    value: bool,
}

impl<'read, 'source> OriginalFlag<'read, 'source> {
    fn read(tree: &'read RecordTree<'source>, selection: &Selection) -> TestResult<Self> {
        let (original, component) = original_component(tree, usize::try_from(selection.ordinal)?)?;
        let Some(flag @ Original::Bool(value)) = component.field(KIOSK_VR_ALIGNMENT) else {
            return Err("the nominated original kiosk flag is not a Bool".into());
        };
        assert_eq!(
            component.type_name(),
            KIOSK,
            "original concrete component type"
        );
        assert_eq!(*value, selection.expected, "original nominated Bool");
        Ok(Self {
            ordinal: selection.ordinal,
            component: original,
            flag,
            value: *value,
        })
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and the private Kiosk selector"]
fn installed_kiosk_flag_matches_original_component() -> TestResult {
    let selection = Selection::read()?;
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let build = archive.build().ok_or("actual Archive Build")?;
    assert_eq!(
        build.change_number(),
        env::var("P4KX_KIOSK_EXPECTED_BUILD")?
    );
    assert_eq!(build.build_id(), env::var("P4KX_KIOSK_EXPECTED_BUILD_ID")?);

    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let index = selection.index(&forge)?;
    let tree = forge.record(index).ok_or("physical Kiosk owner")?.tree()?;
    assert_eq!(
        tree.type_name(),
        "EntityClassDefinition",
        "authored owner type"
    );
    let original = OriginalFlag::read(&tree, &selection)?;
    with_interactions_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(index),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| verify_snapshot(&snapshot, &entry, &tree, &original),
    )??;
    watch.check()?;
    crate::real_archive_verification::print_line(
        "kiosk installed: selectedOwners=1 comparedFlags=1 originalCoordinateChecked=true",
    );
    Ok(())
}

fn verify_snapshot(
    snapshot: &InteractionSnapshot<'_>,
    entry: &DataForgeEntry,
    tree: &RecordTree<'_>,
    original: &OriginalFlag<'_, '_>,
) -> TestResult {
    assert!(
        snapshot
            .rows()
            .iter()
            .all(|row| row.owner().record_index().value() == Some(&tree.record_index())),
        "the SDK rows stay within the selected physical owner"
    );
    let mut matches = snapshot
        .rows()
        .iter()
        .filter(|row| row.owner().component_ordinal().value() == Some(&original.ordinal));
    let row = matches.next().ok_or("selected SDK Kiosk component")?;
    assert!(matches.next().is_none(), "one row per physical component");
    assert_eq!(
        row.source().source_type().value(),
        Some(&KIOSK),
        "SDK concrete Kiosk component"
    );
    let flag = row
        .support_walk_to_align_in_vr()
        .ok_or("selected SDK Kiosk flag")?;
    assert_eq!(
        flag.state(),
        SourceValueState::Present,
        "SDK flag retains its authored presence"
    );
    assert_eq!(
        flag.value(),
        Some(&original.value),
        "SDK flag retains the original Bool"
    );

    let DataForgeOrigin::Archive(source) = entry.origin() else {
        return Err("original DCB Archive origin".into());
    };
    let inventory = serde_json::to_value(snapshot.source_inventory())?;
    let uses = proof::indexed(&inventory, "/uses", "id")?;
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(source), InputScope::Payload)?;
    let encrypted = match entry.origin().encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let trace = Trace::of(tree, &uses, &scope, encrypted);
    let row = serde_json::to_value(row)?;
    trace.header(cell(&row, "ownerRecordIndex")?, "recordIndex", "selector")?;
    trace.check(
        cell(&row, "componentOrdinal")?,
        original.component,
        "conversion",
    )?;
    trace.check(cell(&row, KIOSK_VR_ALIGNMENT)?, original.flag, "payload")
}

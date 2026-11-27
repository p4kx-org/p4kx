// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::worn_gear::{Clothing, ItemRadar, SuitArmor};
use p4kx_export::catalog_queries::equipment::{ComponentOwner, EquipmentSnapshot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ParsedCatalogQueries, ProvenanceMode,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, Build, DataForge, DataForgeEntry, DataForgeOrigin, InputScope,
    RecordTree, SourceEncryption, SourceInput, SourceScope, SourceUses, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde::Serialize;

use super::{
    CLOTHING, RADAR, SUIT, TestResult, body, compare_values, count_macro_values, macro_link,
};
use crate::real_archive_item_durability::{self as proof, Trace};
use crate::real_archive_verification::print_line;

#[path = "worn_gear_sdk_witness_selection.rs"]
mod selection;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and run this test with --ignored"]
fn installed_worn_gear_sdk_matches_original_component_values_and_macro() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())
        .map_err(|_| "open the installed Archive")?;
    let build = archive
        .build()
        .ok_or("the installed Archive needs an exact Build")?;
    let identity = [
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    ];
    assert!(
        identity.iter().all(|part| !part.trim().is_empty()),
        "the worn gear SDK witness requires all four Build identity fields"
    );
    let watch = ArchiveWatch::new(&archive);
    watch
        .check()
        .map_err(|_| "verify Archive before opening the DataForge")?;
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read the installed DataForge")?;
    let original = entry.open().map_err(|_| "open the installed DataForge")?;
    let selected = selection::discover(&original, &watch)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )
    .map_err(|_| "prepare the worn gear SDK session from the decoded DataForge")?
    .with_provenance_mode(ProvenanceMode::Full);
    let sdk = session
        .parsed(None)
        .map_err(|_| "reuse the parsed worn gear SDK session")?;
    let evidence = Evidence::new(&original, entry.origin(), build)?;
    evidence.check(&sdk, selected)?;
    watch
        .check()
        .map_err(|_| "verify Archive after the worn gear SDK proof")?;
    print_line(&format!(
        "Worn gear SDK source witness: {}; three selected component families and one linked macro",
        identity.join(" ")
    ));
    Ok(())
}

struct Evidence<'source> {
    forge: &'source DataForge<'source>,
    build: &'source Build,
    scope: SourceScope,
    encrypted: Option<bool>,
}

impl<'source> Evidence<'source> {
    fn new(
        forge: &'source DataForge<'source>,
        origin: &DataForgeOrigin,
        build: &'source Build,
    ) -> TestResult<Self> {
        let DataForgeOrigin::Archive(source) = origin else {
            return Err("the installed DataForge needs an original Archive Entry".into());
        };
        let encrypted = match origin.encryption() {
            SourceEncryption::Encrypted => Some(true),
            SourceEncryption::Unencrypted => Some(false),
            _ => None,
        };
        let scope = SourceUses::with_compact_coordinates()
            .observe(SourceInput::Archive(source), InputScope::Payload)
            .map_err(|_| "admit the original DataForge Entry")?;
        Ok(Self {
            forge,
            build,
            scope,
            encrypted,
        })
    }

    fn check(
        &self,
        sdk: &ParsedCatalogQueries<'_>,
        [suit, clothing, radar]: [u32; 3],
    ) -> TestResult {
        let target = sdk
            .with_suit_armor(
                CatalogRecordSelection::RecordIndex(suit),
                None,
                |snapshot| {
                    let expected = self
                        .verify(&snapshot, suit, SUIT, SuitArmor::owner)?
                        .ok_or("the selected suit must resolve an original resistance macro")?;
                    let actual = snapshot
                        .rows()
                        .first()
                        .and_then(|row| row.damage_resistance().value())
                        .and_then(|link| link.resolved_index());
                    assert!(
                        actual == Some(expected),
                        "the typed suit link retains the exact target"
                    );
                    Ok::<_, Box<dyn std::error::Error>>(expected)
                },
            )
            .map_err(|_| "read the selected suit SDK rows")??;
        sdk.with_clothing(
            CatalogRecordSelection::RecordIndex(clothing),
            None,
            |snapshot| self.verify(&snapshot, clothing, CLOTHING, Clothing::owner),
        )
        .map_err(|_| "read the selected clothing SDK rows")??;
        sdk.with_item_radars(
            CatalogRecordSelection::RecordIndex(radar),
            None,
            |snapshot| self.verify(&snapshot, radar, RADAR, ItemRadar::owner),
        )
        .map_err(|_| "read the selected item radar SDK rows")??;
        self.verify_macro(sdk, target)
    }

    fn verify<Row: Serialize>(
        &self,
        snapshot: &EquipmentSnapshot<'_, Row>,
        index: u32,
        kind: &str,
        owner: impl Fn(&Row) -> &ComponentOwner,
    ) -> TestResult<Option<u32>> {
        let tree = self.original(index)?;
        assert!(
            snapshot.build() == Some(self.build)
                && matches!(snapshot.availability(), DatasetAvailability::Available),
            "the selected component family is available on the exact Build"
        );
        assert!(
            snapshot.coverage().unreadable_headers.is_empty()
                && snapshot.coverage().unreadable_bodies.is_empty(),
            "the selected owner is readable"
        );
        assert!(
            snapshot.requested_languages().is_empty() && snapshot.available_languages().is_empty(),
            "the direct worn gear query does not admit any locale payload"
        );
        let inventory = serde_json::to_value(snapshot.source_inventory())?;
        let uses = proof::indexed(&inventory, "/uses", "id")?;
        let trace = Trace::of(&tree, &uses, &self.scope, self.encrypted);
        verify_slots(&tree, snapshot.rows(), kind, owner, &trace)
    }

    fn verify_macro(&self, sdk: &ParsedCatalogQueries<'_>, target: u32) -> TestResult {
        let tree = self.original(target)?;
        sdk.with_damage_resistance_macros(
            CatalogRecordSelection::RecordIndex(target),
            None,
            |snapshot| {
                let [row] = snapshot.rows() else {
                    return Err("one exact original resistance macro SDK definition".into());
                };
                let source = row.source();
                assert!(
                    snapshot.build() == Some(self.build)
                        && snapshot.coverage().unreadable_headers.is_empty()
                        && snapshot.coverage().unreadable_bodies.is_empty(),
                    "the referenced SDK definition is readable on the exact Build"
                );
                assert!(
                    source.record_index().value() == Some(&tree.record_index())
                        && source.guid().value() == Some(&tree.guid())
                        && source.source_type().value() == Some(&"DamageResistanceMacro"),
                    "the SDK independently resolves the exact original physical macro definition"
                );
                let inventory = serde_json::to_value(snapshot.source_inventory())?;
                let uses = proof::indexed(&inventory, "/uses", "id")?;
                let trace = Trace::of(&tree, &uses, &self.scope, self.encrypted);
                trace.header(
                    &serde_json::to_value(source.record_index())?,
                    "recordIndex",
                    "selector",
                )?;
                let header = serde_json::to_value(source)?;
                trace.header(
                    header.get("guid").ok_or("original GUID origin")?,
                    "guid",
                    "payload",
                )?;
                let verified = count_macro_values(&serde_json::to_value(row)?, &tree, &trace)?;
                assert!(
                    verified > 0,
                    "the referenced original macro contributes typed values"
                );
                Ok(())
            },
        )
        .map_err(|_| "read the linked macro through its independent SDK route")?
    }

    fn original(&self, index: u32) -> TestResult<RecordTree<'source>> {
        let record = self
            .forge
            .record(index)
            .ok_or("one original physical Record")?;
        record
            .tree()
            .map_err(|_| "read the selected original DataForge Record".into())
    }
}

fn verify_slots<'source, Row: Serialize>(
    tree: &RecordTree<'source>,
    rows: &[Row],
    kind: &str,
    owner: impl Fn(&Row) -> &ComponentOwner,
    trace: &Trace<'_, 'source>,
) -> TestResult<Option<u32>> {
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("the original owner needs a Components array".into());
    };
    let mut returned = rows.iter();
    let mut checked = 0;
    let mut first_target = None;
    for (ordinal, raw) in components.iter().enumerate() {
        let Ok(component) = body(raw) else {
            continue;
        };
        if component.type_name() != kind {
            continue;
        }
        let row = returned
            .next()
            .ok_or("SDK must retain every matching physical slot")?;
        verify_owner(owner(row), tree, u32::try_from(ordinal)?, raw, trace)?;
        let projected = serde_json::to_value(row)?;
        assert!(
            proof::text(&projected, "/sourceType/value")? == kind,
            "same original concrete component declaration"
        );
        checked += compare_values(&projected, component, trace, kind)?;
        if kind == SUIT {
            let target = macro_link(&projected, component, trace)?;
            first_target = first_target.or(Some(target));
        }
    }
    assert!(checked > 0, "one or more original typed cells were checked");
    assert!(
        returned.next().is_none(),
        "the SDK adds no unmatched component occurrence"
    );
    Ok(first_target)
}

fn verify_owner<'source>(
    owner: &ComponentOwner,
    tree: &RecordTree<'source>,
    ordinal: u32,
    raw: &Original<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    assert!(
        owner.record_index().value() == Some(&tree.record_index())
            && owner
                .guid()
                .value()
                .is_some_and(|guid| guid.get() == tree.guid())
            && owner.component_ordinal().value() == Some(&ordinal),
        "the SDK retains this original owner and Components occurrence"
    );
    trace.header(
        &serde_json::to_value(owner.record_index())?,
        "recordIndex",
        "selector",
    )?;
    trace.header(&serde_json::to_value(owner.guid())?, "guid", "payload")?;
    trace.check(
        &serde_json::to_value(owner.component_ordinal())?,
        raw,
        "conversion",
    )
}

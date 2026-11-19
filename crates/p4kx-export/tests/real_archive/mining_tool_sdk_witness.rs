// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::catalog_queries::equipment::consumables::Consumable;
use p4kx_export::catalog_queries::equipment::mining_tools::{AttachableModifier, MiningLaser};
use p4kx_export::catalog_queries::equipment::{ComponentOwner, EquipmentSnapshot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ParsedCatalogQueries,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, Build, DataForge, DataForgeEntry, DataForgeOrigin, InputScope,
    RecordTree, SourceEncryption, SourceInput, SourceScope, SourceUses, StructValue,
    Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};

use super::{TestResult, check_attachable, single};
use crate::real_archive_definition_sdk;
use crate::real_archive_item_durability::{self as proof, Trace};
use crate::real_archive_verification::print_line;

#[path = "mining_tool_sdk_sample.rs"]
mod sample;
#[path = "mining_tool_sdk_values.rs"]
mod values;

const LASER: &str = "SEntityComponentMiningLaserParams";
const ATTACHABLE: &str = "EntityComponentAttachableModifierParams";
const CONSUMABLE: &str = "SCItemConsumableParams";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and run this test with --ignored"]
fn installed_mining_tools_and_consumables_preserve_original_values_and_links() -> TestResult {
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
        "the equipment witness requires all four Build identity fields"
    );
    let watch = ArchiveWatch::new(&archive);
    watch
        .check()
        .map_err(|_| "verify Archive before opening DataForge")?;
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read the installed DataForge")?;
    let original = entry.open().map_err(|_| "open the installed DataForge")?;
    let selected = sample::discover(&original, &watch)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )
    .map_err(|_| "prepare the equipment SDK session from the decoded DataForge")?;
    let sdk = session
        .parsed(None)
        .map_err(|_| "reuse the parsed equipment SDK session")?;
    let context = ComponentEvidence::new(&original, entry.origin(), build)?;
    let links = context.verify_components(&sdk, selected)?;
    let followed = links.globals.len() + links.subtypes.len();
    real_archive_definition_sdk::verify_linked_tool_definitions(
        &sdk,
        &original,
        entry.origin(),
        build,
        links.globals,
        links.subtypes,
    )?;
    watch
        .check()
        .map_err(|_| "verify Archive after the equipment witnesses")?;
    print_line(&format!(
        "Mining tool and consumable SDK source witness: {}; three selected component families and {followed} linked definitions",
        identity.join(" ")
    ));
    Ok(())
}

#[derive(Default)]
struct LinkedDefinitions {
    globals: BTreeSet<u32>,
    subtypes: BTreeSet<u32>,
}

struct ComponentEvidence<'source> {
    forge: &'source DataForge<'source>,
    build: &'source Build,
    scope: SourceScope,
    encrypted: Option<bool>,
}

impl<'source> ComponentEvidence<'source> {
    fn new(
        forge: &'source DataForge<'source>,
        origin: &DataForgeOrigin,
        build: &'source Build,
    ) -> TestResult<Self> {
        let DataForgeOrigin::Archive(source) = origin else {
            return Err("the installed DataForge needs an original Archive Entry".into());
        };
        let scope = SourceUses::with_compact_coordinates()
            .observe(SourceInput::Archive(source), InputScope::Payload)
            .map_err(|_| "admit the original DataForge Entry")?;
        let encrypted = match origin.encryption() {
            SourceEncryption::Encrypted => Some(true),
            SourceEncryption::Unencrypted => Some(false),
            _ => None,
        };
        Ok(Self {
            forge,
            build,
            scope,
            encrypted,
        })
    }

    fn verify_components(
        &self,
        sdk: &ParsedCatalogQueries<'_>,
        [laser, attachable, consumable]: [u32; 3],
    ) -> TestResult<LinkedDefinitions> {
        let mut links = LinkedDefinitions::default();
        self.verify_laser(sdk, laser, &mut links.globals)?;
        self.verify_attachable(sdk, attachable)?;
        self.verify_consumable(sdk, consumable, &mut links.subtypes)?;
        Ok(links)
    }

    fn verify_laser(
        &self,
        sdk: &ParsedCatalogQueries<'_>,
        index: u32,
        globals: &mut BTreeSet<u32>,
    ) -> TestResult {
        sdk.with_mining_lasers(
            CatalogRecordSelection::RecordIndex(index),
            None,
            |snapshot| {
                self.verify(
                    &snapshot,
                    index,
                    LASER,
                    MiningLaser::owner,
                    |row, original, trace| {
                        let wire = serde_json::to_value(row)?;
                        for key in ["throttleMinimum", "throttleLerpSpeed"] {
                            single(&wire, original, trace, key)?;
                        }
                        globals.extend(values::record_link(
                            row.global_params(),
                            original.field("globalParams"),
                            "MiningLaserGlobalParams",
                            trace,
                        )?);
                        Ok(())
                    },
                )
            },
        )
        .map_err(|_| "read the selected mining laser SDK rows")?
    }

    fn verify_attachable(&self, sdk: &ParsedCatalogQueries<'_>, index: u32) -> TestResult {
        sdk.with_attachable_modifiers(
            CatalogRecordSelection::RecordIndex(index),
            None,
            |snapshot| {
                self.verify(
                    &snapshot,
                    index,
                    ATTACHABLE,
                    AttachableModifier::owner,
                    |row, original, trace| {
                        check_attachable(&serde_json::to_value(row)?, original, trace)?;
                        values::attachable_slots(row, original, trace)
                    },
                )
            },
        )
        .map_err(|_| "read the selected attachable modifier SDK rows")?
    }

    fn verify_consumable(
        &self,
        sdk: &ParsedCatalogQueries<'_>,
        index: u32,
        subtypes: &mut BTreeSet<u32>,
    ) -> TestResult {
        sdk.with_consumables(
            CatalogRecordSelection::RecordIndex(index),
            None,
            |snapshot| {
                self.verify(
                    &snapshot,
                    index,
                    CONSUMABLE,
                    Consumable::owner,
                    |row, original, trace| values::consumable(row, original, trace, subtypes),
                )
            },
        )
        .map_err(|_| "read the selected consumable SDK rows")?
    }

    fn verify<Row>(
        &self,
        snapshot: &EquipmentSnapshot<'_, Row>,
        index: u32,
        kind: &str,
        owner: impl Fn(&Row) -> &ComponentOwner,
        mut compare: impl FnMut(&Row, &StructValue<'source>, &Trace<'_, 'source>) -> TestResult,
    ) -> TestResult {
        let tree = self
            .forge
            .record(index)
            .ok_or("one original physical Record")?
            .tree()
            .map_err(|_| "read the selected original DataForge Record")?;
        let coverage = snapshot.coverage();
        assert!(
            snapshot.build() == Some(self.build)
                && matches!(snapshot.availability(), DatasetAvailability::Available)
                && coverage.unreadable_headers.is_empty()
                && coverage.unreadable_bodies.is_empty(),
            "the selected equipment family is readable on this exact Build"
        );
        assert!(
            snapshot.requested_languages().is_empty() && snapshot.available_languages().is_empty(),
            "component lookup does not admit any locale payload"
        );
        let inventory = serde_json::to_value(snapshot.source_inventory())?;
        let uses = proof::indexed(&inventory, "/uses", "id")?;
        let trace = Trace::of(&tree, &uses, &self.scope, self.encrypted);
        let Some(Original::Array(components)) = tree.field("Components") else {
            return Err("the original owner needs a Components array".into());
        };
        let mut rows = snapshot.rows().iter();
        let mut matched = false;
        for (ordinal, raw) in components.iter().enumerate() {
            let Original::Struct(original) = raw else {
                continue;
            };
            if original.type_name() != kind {
                continue;
            }
            let row = rows
                .next()
                .ok_or("SDK must retain every matching physical slot")?;
            verify_owner(owner(row), &tree, u32::try_from(ordinal)?, raw, &trace)?;
            compare(row, original, &trace)?;
            matched = true;
        }
        assert!(
            matched && rows.next().is_none(),
            "exactly the original component occurrences"
        );
        Ok(())
    }
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

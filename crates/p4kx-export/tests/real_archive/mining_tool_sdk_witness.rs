// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::catalog_queries::equipment::consumables::Consumable;
use p4kx_export::catalog_queries::equipment::mining_tools::{AttachableModifier, MiningLaser};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use p4kx_extraction::{Archive, ArchiveWatch, DataForgeEntry};
use p4kx_testkit::{real_archive_key, real_archive_path};

use super::{TestResult, check_attachable, single};
use crate::authored_component_witness::ComponentEvidence;
use crate::real_archive_definition_sdk;
use crate::real_archive_item_durability::Trace;
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

impl ComponentEvidence<'_> {
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
                    &[LASER],
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
                    &[ATTACHABLE],
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
                    &[CONSUMABLE],
                    Consumable::owner,
                    |row, original, trace| values::consumable(row, original, trace, subtypes),
                )
            },
        )
        .map_err(|_| "read the selected consumable SDK rows")?
    }
}

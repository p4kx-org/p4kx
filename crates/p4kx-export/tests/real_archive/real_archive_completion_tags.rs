// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, RecordTree, TreeBuilder, Value,
};
use p4kx_testkit::{real_archive_key, real_archive_pair};
use serde_json::Value as Json;

use crate::real_archive_auto_spawns::original::Locations;
use crate::real_archive_auto_spawns::provenance::Origins;
use crate::real_archive_shop_inventory_v3::verify_build;
use crate::real_archive_verification::print_line;

const PATH: &str = "libs/foundry/records/contracts/contractgenerator/*";
const RESULT: &str = "ContractResult_CompletionTags";
const FLAG: &str = "splitPointsForParty";

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn completion_tag_party_split_matches_both_original_declarations() -> TestResult {
    let Some((live, ptu)) = real_archive_pair()? else {
        return Ok(());
    };
    let key = real_archive_key()?;
    for (path, declared) in [(live, false), (ptu, true)] {
        let archive = Archive::open_with_key(path, Some(key.clone()))?;
        let watch = ArchiveWatch::new(&archive);
        let entry = DataForgeEntry::read(&archive)?;
        let forge = entry.open()?;
        let tree = find_generator(&forge, &watch, declared)?;
        let count = verify_generator(&archive, &entry, &tree)?;
        watch.check()?;
        print_line(&format!(
            "completion-tag result occurrences checked: {count}"
        ));
    }
    Ok(())
}

fn find_generator<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
    declared: bool,
) -> TestResult<RecordTree<'source>> {
    let filter = Filter::new(PATH);
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        watch.poll()?;
        if record.type_name()? != "ContractGenerator" || !filter.matches(record.record_path()?) {
            continue;
        }
        let tree = trees.build(&record)?;
        if source_rows(&tree)?
            .iter()
            .any(|row| row.flag.is_some() == declared)
        {
            return Ok(tree);
        }
    }
    Err("no ContractGenerator with the expected completion-tag field declaration".into())
}

struct OriginalRow {
    index: usize,
    flag: Option<(usize, bool)>,
}

fn source_rows(tree: &RecordTree<'_>) -> TestResult<Vec<OriginalRow>> {
    let locations = Locations::read(tree.root());
    let mut rows = Vec::new();
    let mut fault = None;
    tree.root().visit_owned_values(|visit| {
        if fault.is_some() {
            return;
        }
        let Value::Struct(result) = visit.value() else {
            return;
        };
        if result.type_name() != RESULT || visit.field_name() != "contractResults" {
            return;
        }
        let flag = match result.field(FLAG) {
            Some(value @ Value::Bool(flag)) => match locations.ordinal(value) {
                Ok(index) => Some((index, *flag)),
                Err(error) => {
                    fault = Some(error);
                    return;
                }
            },
            None => None,
            Some(_) => {
                fault = Some("the audited completion-tag result must have a readable Bool".into());
                return;
            }
        };
        rows.push(OriginalRow {
            index: visit.ordinal(),
            flag,
        });
    });
    fault.map_or(Ok(rows), Err)
}

fn verify_generator(
    archive: &Archive,
    entry: &DataForgeEntry,
    tree: &RecordTree<'_>,
) -> TestResult<usize> {
    let options = CatalogOptions {
        filter: Some(&Filter::new(tree.record_path())),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, options.with_full_provenance())
        .map_err(|_| "build selected contract Generator Catalog")?;
    let catalog: Json = serde_json::from_slice(built.json())?;
    verify_build(archive, &catalog)?;
    let origins = Origins::read(&catalog, entry.origin())?;
    let index = tree.record_index();
    let definition = catalog
        .pointer("/definitions/contractGenerators/rows")
        .and_then(Json::as_array)
        .and_then(|rows| {
            rows.iter().find(|row| {
                row.pointer("/recordIndex/value").and_then(Json::as_u64) == Some(u64::from(index))
            })
        })
        .ok_or("the original physical Generator is published")?;
    let published = definition
        .get("completionTagResults")
        .and_then(Json::as_array)
        .ok_or("the completion-tag result table is present")?;
    let original = source_rows(tree)?;
    assert_eq!(
        published.len(),
        original.len(),
        "all original result occurrences retain their order"
    );
    for (row, source) in published.iter().zip(original) {
        let ordinal = source.index.to_string();
        assert_eq!(
            row.pointer("/source/valueIndex").and_then(Json::as_str),
            Some(ordinal.as_str()),
            "the row links to its exact original occurrence"
        );
        let cell = row.get(FLAG).ok_or("the flag has a source carrier")?;
        if let Some((position, flag)) = source.flag {
            assert_eq!(
                cell.get("value").and_then(Json::as_bool),
                Some(flag),
                "the flag matches the original Bool"
            );
            origins.occurrence(cell, index, position, "$value")?;
        } else {
            assert_eq!(
                cell.get("state").and_then(Json::as_str),
                Some("absent"),
                "field absence remains explicit"
            );
            origins.occurrence(cell, index, source.index, "$fields")?;
        }
    }
    Ok(published.len())
}

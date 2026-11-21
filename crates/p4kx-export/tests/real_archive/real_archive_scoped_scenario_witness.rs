// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, RecordTree, Step, TreeBuilder,
    Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use super::{
    GENERATOR, REQUIREMENT, SCENARIO, TestResult, record_index, rows, verify_definitions,
    verify_local_availability,
};
use crate::authored_definition_witness::field;
use crate::real_archive_auto_spawns::{original::Locations, provenance::Origins};
use crate::real_archive_verification::print_line;

#[path = "real_archive_scenario_local_fields.rs"]
mod local;

const GENERATOR_PATH: &str = "libs/foundry/records/contracts/contractgenerator/*";
const SCENARIOS: &str = "/definitions/missionScenarios/rows";
const REQUIREMENTS: &str = "/declarations/contractScenarioRequirements/rows";
const LOCAL_GENERATORS: &str = "/configurations/localContractGenerators/rows";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn selected_original_scenario_and_local_generator_slots_match_catalog() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let mut scenario = None;
    let mut checked = BTreeSet::new();
    for tree in select_generators(&forge, &watch)?.into_iter().flatten() {
        if checked.insert(tree.record_index()) {
            let catalog = build(&archive, &entry, &Filter::new(tree.record_path()))?;
            let origins = Origins::read(&catalog, entry.origin())?;
            scenario = scenario.or(compare_generator(&forge, &catalog, &origins, &tree)?);
        }
    }
    if checked.is_empty() {
        print_line("ContractGenerator: no original scenario or local configuration slots observed");
    }
    verify_scenario(&forge, &archive, &entry, scenario)?;
    watch
        .check()
        .map_err(|_| "Archive changed during scenario verification".into())
}

fn select_generators<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> TestResult<[Option<RecordTree<'source>>; 2]> {
    let path = Filter::new(GENERATOR_PATH);
    let mut builder = TreeBuilder::new();
    let mut scenario = None;
    let mut local = None;
    let mut fallback = None;
    for record in forge.records() {
        watch.poll().map_err(|_| "Archive changed")?;
        if record.type_name()? != GENERATOR || !path.matches(record.record_path()?) {
            continue;
        }
        let tree = builder
            .build(&record)
            .map_err(|_| "read candidate Generator Record")?;
        let mut has_target = false;
        let mut has_requirements = false;
        visit_requirements(&tree, |_, _, value| {
            has_target |= target(value).is_some();
            has_requirements = true;
            Ok(())
        })?;
        let has_local = local::handlers(&tree).any(local::populated);
        match (has_target, has_local) {
            (true, true) => return Ok([Some(tree), None]),
            (true, false) if scenario.is_none() => scenario = Some(tree),
            (false, true) if local.is_none() => local = Some(tree),
            _ if fallback.is_none()
                && (has_requirements || local::handlers(&tree).next().is_some()) =>
            {
                fallback = Some(tree);
            }
            _ => {}
        }
        if scenario.is_some() && local.is_some() {
            break;
        }
    }
    Ok([scenario.or(fallback), local])
}

fn visit_requirements<'source>(
    tree: &RecordTree<'source>,
    mut compare: impl FnMut(usize, Option<usize>, &Original<'source>) -> TestResult,
) -> TestResult {
    let mut parents = Vec::new();
    let mut result = Ok(());
    tree.root().visit_owned_values(|visit| {
        if result.is_err() {
            return;
        }
        let depth = visit.steps().len();
        while parents.last().is_some_and(|(level, _, _)| *level >= depth) {
            parents.pop();
        }
        if matches!(visit.steps().last(), Some(Step::Field(REQUIREMENT))) {
            let (owner, index) = parents.last().map_or_else(
                || (None, tree.root().struct_index()),
                |(_, ordinal, index)| (Some(*ordinal), *index),
            );
            result = (|| {
                if requirement_owner(tree, index)? {
                    compare(visit.ordinal(), owner, visit.value())?;
                }
                Ok(())
            })();
        }
        if let Original::Struct(data) = visit.value() {
            parents.push((depth, visit.ordinal(), data.struct_index()));
        }
    });
    result
}

fn requirement_owner(tree: &RecordTree<'_>, index: u32) -> TestResult<bool> {
    let mut actual = tree
        .struct_at(index)
        .ok_or("original requirement owner type")?;
    let mut seen = BTreeSet::new();
    while seen.insert(actual.index()) {
        if matches!(
            actual.name()?,
            GENERATOR | "ContractGeneratorHandlerBase" | "ContractBase"
        ) {
            return Ok(true);
        }
        let Some(parent) = actual.parent() else {
            return Ok(false);
        };
        actual = parent;
    }
    Err("cyclic original requirement owner ancestry".into())
}

fn target(value: &Original<'_>) -> Option<u32> {
    let Original::Array(values) = value else {
        return None;
    };
    values.iter().find_map(|value| match value {
        Original::Reference(link) if link.type_name() == SCENARIO => Some(link.record_index()),
        _ => None,
    })
}

fn compare_generator(
    forge: &DataForge<'_>,
    catalog: &Value,
    origins: &Origins<'_>,
    tree: &RecordTree<'_>,
) -> TestResult<Option<u32>> {
    let index = tree.record_index();
    let definition = rows(catalog, "/definitions/contractGenerators/rows")?
        .iter()
        .find(|row| {
            row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(u64::from(index))
        })
        .ok_or("selected original Generator definition")?;
    let input = field(definition, "inputRef")?;
    let requirement_rows = rows(catalog, REQUIREMENTS)?;
    let local_rows = by_owner(rows(catalog, LOCAL_GENERATORS)?, index, input)?;
    verify_local_availability(forge, catalog)?;
    let actual = by_owner(requirement_rows, index, input)?;
    let mut rows = actual.iter();
    let mut target_index = None;
    let check = local::Check::new(origins, tree);
    visit_requirements(tree, |ordinal, owner, value| {
        let row = rows
            .next()
            .ok_or("missing original requirement occurrence")?;
        let header = field(row, "source")?;
        assert_eq!(
            value_index(field(header, "scenariosValue")?)?,
            ordinal,
            "the original requirement array position"
        );
        let cell = field(header, "owner")?;
        let actual_owner = (!cell.is_null()).then(|| value_index(cell)).transpose()?;
        assert_eq!(
            actual_owner, owner,
            "the original inherited requirement owner"
        );
        check.links(forge, SCENARIO, Some(value), field(row, "scenarios")?)?;
        target_index = target_index.or_else(|| target(value));
        Ok(())
    })?;
    assert!(
        rows.next().is_none(),
        "every requirement has its original occurrence"
    );
    local::compare(forge, &check, tree, &local_rows)?;
    print_line(&format!(
        "ContractGenerator: {} original scenario requirement lists, {} local Handler rows checked",
        actual.len(),
        local_rows.len()
    ));
    Ok(target_index)
}

fn by_owner<'row>(rows: &'row [Value], index: u32, input: &Value) -> TestResult<Vec<&'row Value>> {
    let mut matched = Vec::new();
    for row in rows {
        let source = field(row, "source")?;
        if record_index(field(source, "recordIndex")?)? == index {
            assert!(
                field(source, "inputRef")? == input,
                "typed row joins the original Generator input"
            );
            matched.push(row);
        }
    }
    Ok(matched)
}

fn value_index(row: &Value) -> TestResult<usize> {
    Ok(field(row, "valueIndex")?
        .as_str()
        .ok_or("original occurrence position")?
        .parse()?)
}

fn verify_scenario(
    forge: &DataForge<'_>,
    archive: &Archive,
    entry: &DataForgeEntry,
    target: Option<u32>,
) -> TestResult {
    let selected = target.and_then(|index| forge.record(index)).or_else(|| {
        forge
            .records()
            .find(|record| record.type_name().ok() == Some(SCENARIO))
    });
    let Some(record) = selected else {
        print_line("MissionScenario: no original definition observed");
        return Ok(());
    };
    let filter = Filter::new(record.record_path()?);
    let catalog = build(archive, entry, &filter)?;
    let definitions = rows(&catalog, SCENARIOS)?;
    assert!(
        !definitions.is_empty(),
        "the original MissionScenario target is exported"
    );
    verify_definitions(forge, definitions, Some(&filter))?;
    print_line(if target.is_some() {
        "MissionScenario: original resolved requirement and separately exported physical target agree"
    } else {
        "MissionScenario: original definition checked; no resolved requirement observed"
    });
    Ok(())
}

fn build(archive: &Archive, entry: &DataForgeEntry, filter: &Filter) -> TestResult<Value> {
    let options = CatalogOptions {
        filter: Some(filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, &options)
        .map_err(|_| "build scoped scenario catalog")?;
    let catalog: Value =
        serde_json::from_slice(built.json()).map_err(|_| "read scoped catalog JSON")?;
    crate::real_archive_shop_inventory_v3::verify_build(archive, &catalog)?;
    Ok(catalog)
}

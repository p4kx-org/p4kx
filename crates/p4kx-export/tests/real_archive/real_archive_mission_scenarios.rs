// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, TreeBuilder, Value as ForgeValue,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::authored_definition_witness::{field, verify_array, verify_link};
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_verification::print_line;

const SCENARIO: &str = "MissionScenario";
const GENERATOR: &str = "ContractGenerator";
const REQUIREMENT: &str = "required_active_scenarios";

#[path = "real_archive_scoped_scenario_witness.rs"]
mod scoped;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn authored_mission_scenarios_match_their_physical_records_and_requirement_slots() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build mission scenario catalog")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let definitions = rows(&catalog, "/definitions/missionScenarios/rows")?;
    let requirements = rows(&catalog, "/declarations/contractScenarioRequirements/rows")?;
    verify_definitions(&forge, definitions, None)?;
    verify_requirements(&forge, requirements)?;
    verify_local_generators(&forge, &catalog)?;
    watch
        .check()
        .map_err(|_| "Archive changed during scenario verification")?;
    print_line(&format!(
        "mission scenarios: {} physical definitions and {} original requirement lists checked",
        definitions.len(),
        requirements.len()
    ));
    Ok(())
}

fn rows<'row>(catalog: &'row Value, path: &str) -> TestResult<&'row [Value]> {
    catalog
        .pointer(path)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("missing scenario table {path}").into())
}

fn verify_definitions(
    forge: &DataForge<'_>,
    rows: &[Value],
    filter: Option<&Filter>,
) -> TestResult {
    let paths = Filter::new("libs/foundry/records/*");
    let mut emitted = BTreeMap::new();
    for row in rows {
        let index = record_index(consumer::present(field(row, "recordIndex")?)?)?;
        if emitted.insert(index, row).is_some() {
            return Err("duplicate physical scenario definition".into());
        }
    }
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        if record.type_name().ok() != Some(SCENARIO)
            || !record.record_path().is_ok_and(|path| {
                paths.matches(path) && filter.is_none_or(|filter| filter.matches(path))
            })
        {
            continue;
        }
        let row = emitted
            .remove(&record.index())
            .ok_or("missing physical scenario definition")?;
        assert!(
            consumer::present(field(row, "guid")?)?.as_str()
                == Some(record.guid().to_string().as_str()),
            "the emitted definition identifies the exact Record"
        );
        let tree = trees
            .build(&record)
            .map_err(|_| "read original scenario definition")?;
        let original = tree.root().fields();
        let authored = field(field(row, "authoredFields")?, "roots")?
            .as_array()
            .ok_or("authored scenario fields")?;
        assert_eq!(
            authored.len(),
            original.len(),
            "every original root field is retained"
        );
        for (root, source) in authored.iter().zip(original) {
            assert_eq!(
                consumer::present(field(root, "name")?)?.as_str(),
                Some(source.name()),
                "scenario fields retain their original declaration order"
            );
        }
    }
    assert!(
        emitted.is_empty(),
        "every emitted row matches an original scenario Record"
    );
    Ok(())
}

fn verify_requirements(forge: &DataForge<'_>, rows: &[Value]) -> TestResult {
    let mut records = BTreeMap::<u32, BTreeMap<usize, &Value>>::new();
    for row in rows {
        let source = field(row, "source")?;
        let index = record_index(field(source, "recordIndex")?)?;
        let ordinal = field(field(source, "scenariosValue")?, "valueIndex")?
            .as_str()
            .ok_or("scenario source index")?
            .parse()?;
        if records
            .entry(index)
            .or_default()
            .insert(ordinal, row)
            .is_some()
        {
            return Err("duplicate original scenario requirement occurrence".into());
        }
    }
    let mut trees = TreeBuilder::new();
    for (index, mut expected) in records {
        let record = forge.record(index).ok_or("original Generator Record")?;
        assert_eq!(
            record.type_name()?,
            GENERATOR,
            "requirements link to a physical Generator"
        );
        let tree = trees
            .build(&record)
            .map_err(|_| "read original scenario requirement owner")?;
        let mut result = Ok(());
        tree.root().visit_owned_values(|visit| {
            if result.is_err() {
                return;
            }
            if let Some(row) = expected.remove(&visit.ordinal()) {
                result = (|| {
                    assert_eq!(
                        visit.field_name(),
                        REQUIREMENT,
                        "the typed row points to the actual requirement member"
                    );
                    verify_array(
                        Some(visit.value()),
                        field(row, "scenarios")?,
                        |value, cell| verify_link(forge, SCENARIO, value, cell),
                    )
                })();
            }
        });
        result?;
        assert!(
            expected.is_empty(),
            "every typed list resolves to an original array occurrence"
        );
    }
    Ok(())
}

fn record_index(value: &Value) -> TestResult<u32> {
    Ok(u32::try_from(
        value.as_u64().ok_or("physical Record index")?,
    )?)
}

fn verify_local_availability(forge: &DataForge<'_>, catalog: &Value) -> TestResult {
    let table = field(field(catalog, "configurations")?, "localContractGenerators")?;
    let mut declared = false;
    for structure in forge.structs() {
        match structure.name()? {
            "LocalContractGeneratorParams" => declared = true,
            "ContractAvailability" => {
                for property in structure.properties() {
                    declared |= property.name()? == "missionBoardAvailability";
                }
            }
            _ => {}
        }
    }
    assert_eq!(
        field(table, "availability")?.as_str(),
        Some(if declared { "available" } else { "absent" }),
        "availability reflects the original generator and board declarations"
    );
    Ok(())
}

fn verify_local_generators(forge: &DataForge<'_>, catalog: &Value) -> TestResult {
    verify_local_availability(forge, catalog)?;
    let mut trees = TreeBuilder::new();
    for row in rows(catalog, "/configurations/localContractGenerators/rows")? {
        let source = field(row, "source")?;
        let index = record_index(field(source, "recordIndex")?)?;
        let ordinal = field(field(source, "handler")?, "valueIndex")?
            .as_str()
            .ok_or("original Handler index")?
            .parse::<usize>()?;
        let record = forge.record(index).ok_or("original Generator Record")?;
        assert_eq!(
            record.type_name()?,
            GENERATOR,
            "the exact Generator owns the Handler"
        );
        let tree = trees
            .build(&record)
            .map_err(|_| "read original Generator")?;
        let mut check = None;
        tree.root().visit_owned_values(|visit| {
            if visit.ordinal() == ordinal {
                check = Some(verify_local_handler(visit.value(), row));
            }
        });
        check.ok_or("missing original Handler occurrence")??;
    }
    Ok(())
}

fn verify_local_handler(source: &ForgeValue<'_>, row: &Value) -> TestResult {
    let ForgeValue::Struct(handler) = source else {
        return Err("original Handler slot is not a structure".into());
    };
    if let Some(ForgeValue::Struct(params)) = handler.field("localGeneratorParams")
        && let Some(ForgeValue::Enum(scope)) = params.field("scope")
    {
        let cell = field(
            consumer::present(field(row, "localGeneratorParams")?)?,
            "scope",
        )?;
        assert_eq!(
            consumer::present(cell)?.as_str(),
            Some(*scope),
            "the scope is the original Handler enum"
        );
    }
    if let Some(ForgeValue::Struct(availability)) = handler.field("defaultAvailability")
        && let Some(ForgeValue::Enum(board)) = availability.field("missionBoardAvailability")
    {
        let defaults = consumer::present(field(row, "defaultAvailability")?)?;
        assert_eq!(
            consumer::present(field(defaults, "missionBoardAvailability")?)?.as_str(),
            Some(*board),
            "the board setting is the original Handler enum"
        );
    }
    Ok(())
}

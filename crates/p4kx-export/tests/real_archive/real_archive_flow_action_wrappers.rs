// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, Filter, TreeBuilder, Value,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value as Json;

#[path = "real_archive_flow_action_wrapper_graph.rs"]
mod graph;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const TEMPLATE: &str = "ContractTemplate";
const FILTER: &str = "libs/foundry/records/contracts/contracttemplates/*";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn selected_flow_action_wrappers_match_the_original_owner_graph() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read the owning DataForge")?;
    let filter = Filter::new(FILTER);
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build selected Template diagnostic export")?;
    let catalog: Json = serde_json::from_slice(built.json()).map_err(|_| "read catalog JSON")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let counts = compare(&forge, &filter, &watch, &catalog)?;
    watch
        .check()
        .map_err(|_| "Archive changed during verification")?;
    if std::env::var_os("P4KX_EXPECT_FLOW_ACTION_WRAPPERS").is_some()
        && (counts.multiple == 0 || counts.random == 0)
    {
        return Err("configured Build did not exercise both original action wrappers".into());
    }
    Ok(())
}

fn compare(
    forge: &DataForge<'_>,
    filter: &Filter,
    watch: &ArchiveWatch,
    catalog: &Json,
) -> TestResult<graph::Counts> {
    let mut counts = graph::Counts::default();
    let mut trees = TreeBuilder::new();
    let definitions = graph::rows(catalog, "/definitions/contractTemplates/rows")?;
    let flows = graph::rows(catalog, "/configurations/missionFlows/rows")?;
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during verification")?;
        let path = record
            .record_path()
            .map_err(|_| "read original Record path")?;
        if !filter.matches(path) || record.type_name().ok() != Some(TEMPLATE) {
            continue;
        }
        let tree = trees
            .build(&record)
            .map_err(|_| "read selected original Template")?;
        let roots = original_actions(tree.field("missionFlow"));
        if !roots.iter().flatten().any(|value| graph::is_wrapper(value)) {
            continue;
        }
        let index = u64::from(record.index());
        let guid = record.guid().to_string();
        let definition = definitions
            .iter()
            .find(|row| {
                row.pointer("/recordIndex/value").and_then(Json::as_u64) == Some(index)
                    && row.pointer("/guid/value").and_then(Json::as_str) == Some(guid.as_str())
            })
            .ok_or("affected Template has its physical definition")?;
        let flow = flows
            .iter()
            .find(|row| {
                row.pointer("/owner/type").and_then(Json::as_str) == Some("contractTemplate")
                    && row.pointer("/owner/recordIndex").and_then(Json::as_u64) == Some(index)
                    && row.pointer("/owner/recordGuid").and_then(Json::as_str)
                        == Some(guid.as_str())
                    && row.pointer("/selection/type").and_then(Json::as_str) == Some("selected")
            })
            .ok_or("affected original Template has its selected Flow")?;
        let values = graph::rows(definition, "/source/values")?;
        compare_roots(&roots, flow, values, &mut counts)?;
    }
    Ok(counts)
}

fn compare_roots(
    original: &[Option<&Value<'_>>],
    flow: &Json,
    values: &[Json],
    counts: &mut graph::Counts,
) -> TestResult {
    let triggers = graph::rows(flow, "/triggers")?;
    if triggers.len() != original.len() {
        return Err("actual selected original trigger count differs".into());
    }
    for (root, trigger) in original.iter().zip(triggers) {
        if let Some(root) = (*root).filter(|node| graph::is_wrapper(node)) {
            let trigger = graph::follow(values, trigger)?;
            let action = trigger
                .pointer("/trigger/action")
                .ok_or("original trigger action link")?;
            graph::check(root, graph::follow(values, action)?, values, counts)?;
        }
    }
    Ok(())
}

fn original_actions<'view, 'source>(
    flow: Option<&'view Value<'source>>,
) -> Vec<Option<&'view Value<'source>>> {
    let Some(Value::Struct(flow)) = first_struct(flow) else {
        return Vec::new();
    };
    let Some(Value::Array(triggers)) = flow.field("triggers") else {
        return Vec::new();
    };
    triggers
        .iter()
        .filter_map(|trigger| match trigger {
            Value::Struct(trigger) => Some(first_struct(trigger.field("action"))),
            _ => None,
        })
        .collect()
}

fn first_struct<'view, 'source>(
    original: Option<&'view Value<'source>>,
) -> Option<&'view Value<'source>> {
    match original? {
        node @ Value::Struct(_) => Some(node),
        Value::Array(values) => values
            .iter()
            .find(|value| matches!(value, Value::Struct(_))),
        _ => None,
    }
}

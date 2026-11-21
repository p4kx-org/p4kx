// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_export::{CatalogPublicationMode, Filter};
use p4kx_extraction::{DataForge, DataForgeEntry, Value as Raw};
use p4kx_testkit::DataForgeBuilder;
use serde_json::Value;

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::crafting_detail;
use crate::quality_v3_fixture_schema as schema;
use crate::quality_v3_fixture_values as values;

const LINK: &str = ".qualityQuantization.qualityQuantizationRecord";
pub(super) const WIDE: usize = 4_097;

pub(super) struct Original {
    pub(super) quantizer: u32,
    pub(super) wide: u32,
    pub(super) narrow: u32,
    pub(super) wide_count: usize,
}

pub(super) fn observed(
    wide_count: usize,
    filter: Option<&Filter>,
    origin: DcbOrigin,
) -> TestResult<(Value, Original)> {
    let database = database(wide_count);
    let (_directory, archive) = common::open(&database, origin)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let source = original(&forge, wide_count)?;
    let mut options = common::options();
    options.filter = filter;
    options.publication_mode = CatalogPublicationMode::Normal;
    let built = p4kx_export::build_catalog_from(&archive, &entry, &options)?;
    assert!(
        built.summary().is_complete(),
        "every fixture source is readable"
    );
    let catalog = serde_json::from_slice(built.json())?;
    Ok((catalog, source))
}

fn database(wide_count: usize) -> DataForgeBuilder {
    schema::schema()
        .record(values::quantization(
            values::QUANTIZATION,
            "Quantizer",
            "quantizer",
            values::bands(vec![values::band(0, 9, 3)]),
        ))
        .record(values::resource(
            values::RESOURCE_A,
            "wide",
            (0..wide_count)
                .map(|_| values::property(Some(values::QUANTIZATION)))
                .collect(),
        ))
        .record(values::resource(
            values::RESOURCE_B,
            "narrow",
            vec![values::property(Some(values::QUANTIZATION))],
        ))
}

fn original(forge: &DataForge<'_>, wide_count: usize) -> TestResult<Original> {
    let quantizer = forge
        .record_by_guid(crafting_detail::guid(values::QUANTIZATION))
        .ok_or("original quantizer")?;
    let wide = links(forge, values::RESOURCE_A, wide_count, quantizer.index())?;
    let narrow = links(forge, values::RESOURCE_B, 1, quantizer.index())?;
    assert_ne!(
        wide, narrow,
        "the two referrers have separate physical Records"
    );
    assert!(
        wide < narrow,
        "the later narrow resource is independently selected"
    );
    Ok(Original {
        quantizer: quantizer.index(),
        wide,
        narrow,
        wide_count,
    })
}

fn links(forge: &DataForge<'_>, guid: u128, count: usize, target: u32) -> TestResult<u32> {
    let record = forge
        .record_by_guid(crafting_detail::guid(guid))
        .ok_or("original resource")?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the decoded tree belongs to the actual Resource Record"
    );
    let Some(Raw::Array(values)) = tree.root().field("properties") else {
        return Err("original Resource properties".into());
    };
    assert_eq!(values.len(), count, "the exact authored slot count");
    for value in values {
        let Raw::Struct(property) = value else {
            return Err("actual original property".into());
        };
        assert_eq!(
            property.type_name(),
            "ResourceTypeCraftingData",
            "the original quality property has the declared source type"
        );
        let Some(Raw::Struct(source)) = property.field("qualityQuantization") else {
            return Err("authored quantization".into());
        };
        let Some(Raw::Reference(link)) = source.field("qualityQuantizationRecord") else {
            return Err("authored quantizer Reference".into());
        };
        assert_eq!(
            link.record_index(),
            target,
            "the original Reference targets the exact physical quantizer"
        );
        assert!(
            link.location().is_none(),
            "the original Record Reference carries no location override"
        );
    }
    Ok(record.index())
}

pub(super) fn relationships(catalog: &Value, cell: &Value) -> TestResult<BTreeSet<(u32, usize)>> {
    let references = cell["sourceUsesRefs"]
        .as_array()
        .ok_or("actual immediate uses")?;
    let ids = references
        .iter()
        .map(|reference| reference.as_str().ok_or("source-use reference"))
        .collect::<Result<BTreeSet<_>, _>>()?;
    assert_eq!(
        ids.len(),
        references.len(),
        "the actual immediate uses are unique"
    );
    let sources = catalog
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("physical sources")?;
    let sources: BTreeMap<_, _> = sources
        .iter()
        .filter_map(|source| source["id"].as_str().map(|id| (id, source)))
        .collect();
    let inventory = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source-use inventory")?;
    let mut used = BTreeSet::new();
    let mut closed = 0;
    for row in inventory {
        let id = row["id"].as_str().ok_or("source-use id")?;
        if !ids.contains(id) {
            continue;
        }
        closed += 1;
        if let Some(relationship) = relationship(row, &sources)? {
            used.insert(relationship);
        }
    }
    assert_eq!(
        closed,
        ids.len(),
        "all immediate references have actual inventory rows"
    );
    Ok(used)
}

fn relationship(row: &Value, sources: &BTreeMap<&str, &Value>) -> TestResult<Option<(u32, usize)>> {
    let location = &row["location"];
    let Some(ordinal) = location["field"]
        .as_str()
        .and_then(|path| path.strip_prefix("properties["))
        .and_then(|path| path.strip_suffix(LINK))
        .and_then(|ordinal| ordinal.strip_suffix(']'))
    else {
        return Ok(None);
    };
    assert_eq!(
        row["role"], "selector",
        "the retained relationship selected the derived result"
    );
    assert_eq!(
        row["scope"], "payload",
        "the relationship comes from the original payload"
    );
    let physical = row["source"]
        .as_str()
        .and_then(|id| sources.get(id))
        .ok_or("observed physical input")?;
    assert_eq!(
        physical["kind"], "archiveEntry",
        "the evidence resolves to the exact physical archive entry"
    );
    assert_eq!(
        physical.pointer("/path/value").and_then(Value::as_str),
        Some("Data\\Game2.dcb"),
        "the Reference was decoded from the original Game2 DataForge"
    );
    let record = location["recordIndex"]
        .as_u64()
        .ok_or("exact original source Record")?;
    Ok(Some((u32::try_from(record)?, ordinal.parse()?)))
}

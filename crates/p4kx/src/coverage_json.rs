// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use p4kx_extraction::{
    ComponentCoverage, EntryCoverage, FieldOrigins, ObjectCoverage, ObservedCoverage, SampleExport,
    SourceInventory, TypeCoverage,
};
use serde::Serialize;

#[derive(Serialize)]
struct Document<'a> {
    archive: String,
    complete: bool,
    unreadable_entities: u64,
    types: Vec<TypeJson<'a>>,
    components: Vec<ComponentJson<'a>>,
    starmap_types: Vec<ObjectJson<'a>>,
    entries: Vec<EntryJson<'a>>,
    #[serde(rename = "fieldOrigins")]
    field_origins: &'a FieldOrigins,
    provenance: &'a SourceInventory,
}

#[derive(Serialize)]
struct TypeJson<'a> {
    area: &'a str,
    name: &'a str,
    candidate: bool,
    defined: bool,
    records: u64,
    unreadable_records: u64,
    error_values: u64,
    sample: Option<SampleJson<'a>>,
}

#[derive(Serialize)]
struct SampleJson<'a> {
    record_path: &'a str,
    xml_bytes: usize,
    json_bytes: usize,
    error: Option<&'a str>,
}

#[derive(Serialize)]
struct ComponentJson<'a> {
    area: &'a str,
    name: &'a str,
    defined: bool,
    entities: u64,
    instances: u64,
    reachable: bool,
}

#[derive(Serialize)]
struct ObjectJson<'a> {
    area: &'a str,
    name: &'a str,
    records: u64,
    templates: u64,
    sample: Option<SampleJson<'a>>,
}

impl<'a> From<&'a ObjectCoverage> for ObjectJson<'a> {
    fn from(found: &'a ObjectCoverage) -> Self {
        Self {
            area: &found.area,
            name: &found.type_name,
            records: found.records,
            templates: found.templates,
            sample: found.sample.as_ref().map(SampleJson::from),
        }
    }
}

#[derive(Serialize)]
struct EntryJson<'a> {
    area: &'a str,
    folder: &'a str,
    entries: u64,
    converted: u64,
    text: u64,
    failures: Vec<FailureJson<'a>>,
}

#[derive(Serialize)]
struct FailureJson<'a> {
    path: &'a str,
    reason: &'a str,
}

impl<'a> From<&'a TypeCoverage> for TypeJson<'a> {
    fn from(found: &'a TypeCoverage) -> Self {
        Self {
            area: &found.area,
            name: &found.name,
            candidate: found.candidate,
            defined: found.defined,
            records: found.records,
            unreadable_records: found.unreadable_records,
            error_values: found.error_values,
            sample: found.sample.as_ref().map(SampleJson::from),
        }
    }
}

impl<'a> From<&'a SampleExport> for SampleJson<'a> {
    fn from(sample: &'a SampleExport) -> Self {
        Self {
            record_path: &sample.record_path,
            xml_bytes: sample.xml_bytes,
            json_bytes: sample.json_bytes,
            error: sample.error.as_deref(),
        }
    }
}

impl<'a> From<&'a ComponentCoverage> for ComponentJson<'a> {
    fn from(found: &'a ComponentCoverage) -> Self {
        Self {
            area: &found.area,
            name: &found.name,
            defined: found.defined,
            entities: found.entities,
            instances: found.instances,
            reachable: found.reachable,
        }
    }
}

impl<'a> From<&'a EntryCoverage> for EntryJson<'a> {
    fn from(found: &'a EntryCoverage) -> Self {
        Self {
            area: &found.area,
            folder: &found.folder,
            entries: found.entries,
            converted: found.converted,
            text: found.text,
            failures: found
                .failures
                .iter()
                .map(|(path, reason)| FailureJson { path, reason })
                .collect(),
        }
    }
}

/// Prints the report as one line of JSON.
pub(crate) fn print(archive: &Path, observed: &ObservedCoverage) -> io::Result<()> {
    let report = observed.report();
    crate::json::print(&Document {
        archive: crate::json::path(archive),
        complete: report.is_complete(),
        unreadable_entities: report.unreadable_entities,
        types: report.types.iter().map(TypeJson::from).collect(),
        components: report.components.iter().map(ComponentJson::from).collect(),
        starmap_types: report.objects.iter().map(ObjectJson::from).collect(),
        entries: report.entries.iter().map(EntryJson::from).collect(),
        field_origins: observed.fields(),
        provenance: observed.inventory(),
    })
}

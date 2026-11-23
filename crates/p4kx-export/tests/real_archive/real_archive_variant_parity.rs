// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::io::{BufReader, BufWriter};
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogError, CatalogOptions, Compression, SCHEMA_VERSION, write_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, FieldOrigin, Filter};
use serde_json::Value;

use super::verify_build;
use crate::real_archive_catalog_pipe as pipe;
use crate::real_archive_catalog_table_parity::{Origins, TestResult};

const USES: &str = "sourceUsesRefs";
const SHARED_SETS: &str = "/provenance/variantSourceUseSets";

pub(super) fn scoped_normal(
    archive: &Archive,
    entry: &DataForgeEntry,
    filter: &Filter,
    table: &str,
    origins: &Origins,
) -> TestResult<Value> {
    let options = CatalogOptions {
        filter: Some(filter),
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let sections = [
        "/schemaVersion",
        "/source",
        "/exportHealth/publicationMode",
        table,
        SHARED_SETS,
    ];
    let (_, document) = pipe::transfer(
        |reader| origins.read_scope(BufReader::new(reader), &sections),
        |writer| {
            let (summary, sink) =
                write_catalog_from(archive, entry, &options, Compression::None, || {
                    Ok(BufWriter::new(writer))
                })?;
            let writer = sink
                .into_inner()
                .map_err(|error| CatalogError::Write(error.into_error()))?;
            Ok((summary, writer))
        },
    )
    .map_err(|_| "the streamed exact Normal Catalog scope did not complete")?;
    verify_build(archive, &document)?;
    if document.get("schemaVersion").and_then(Value::as_u64) != Some(u64::from(SCHEMA_VERSION))
        || document
            .pointer("/exportHealth/publicationMode")
            .and_then(Value::as_str)
            != Some("normal")
    {
        return Err("the scoped producer lacks the current normal Catalog wire contract".into());
    }
    Ok(document)
}

pub(super) fn verify_expanded_origin(
    wire: &Value,
    set: Option<&Value>,
    origin: &FieldOrigin,
) -> TestResult {
    let mut expanded = BTreeSet::new();
    for carrier in std::iter::once(wire).chain(set) {
        let refs = carrier
            .get(USES)
            .and_then(Value::as_array)
            .ok_or("missing variant source-use list")?;
        for reference in refs {
            let id = reference
                .as_str()
                .ok_or("non-string variant source-use reference")?;
            if !expanded.insert(id) {
                return Err("the variant's pooled and local sources overlap or repeat".into());
            }
        }
    }
    if !expanded.into_iter().eq(origin
        .source_uses_refs()
        .iter()
        .map(p4kx_extraction::SourceUseRef::as_str))
    {
        return Err(
            "expanding pooled and local variant sources differs from the SDK origin".into(),
        );
    }
    Ok(())
}

pub(super) fn selected_set<'catalog>(
    normal: &'catalog Value,
    row: &Value,
) -> TestResult<Option<&'catalog Value>> {
    let reference = row
        .pointer("/variantOf/sourceUseSetRef")
        .ok_or("the Normal row lacks its variant source-use-set relation")?;
    if reference.is_null() {
        return Ok(None);
    }
    let id = reference
        .as_u64()
        .ok_or("the Normal row has an invalid variant source-use-set reference")?;
    let mut matching = normal
        .pointer(SHARED_SETS)
        .and_then(Value::as_array)
        .ok_or("the Normal producer lacks pooled variant sources")?
        .iter()
        .filter(|row| row.get("id").and_then(Value::as_u64) == Some(id));
    let selected = matching
        .next()
        .ok_or("the Normal producer missed the selected variant set")?;
    if matching.next().is_some() {
        return Err("the Normal producer repeated the selected variant set".into());
    }
    Ok(Some(selected))
}

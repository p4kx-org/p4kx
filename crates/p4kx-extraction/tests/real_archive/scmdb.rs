// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! DataForge values against SCMDB data dumps of the same Build saved by hand (ADR-0267); runs
//! with `--ignored` when `P4KX_DATA_P4K` names an Archive and `P4KX_REF_SCMDB` the folder.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use p4kx_extraction::{Archive, DataForge, DataForgeEntry, Guid, JsonLayout, Record, RecordJson};
use p4kx_testkit::{real_archive_key, real_archive_path, reference_scmdb_path};
use rayon::prelude::*;
use serde_json::Value;

use super::scmdb_compare::{
    BLUEPRINTS, ContractFields, Dumps, MERGED, MINING, P4kx, collect_contracts, compare,
    dump_file_name, find_dump_version,
};
use super::{SendError, report};

/// The disagreements per category the failure message shows.
const EXAMPLES_PER_CATEGORY: usize = 5;

/// The text in the type or record path of every Record that holds contracts, in lower case.
const CONTRACT_MARK: &str = "contract";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn dataforge_values_agree_with_scmdb_dumps_of_the_same_build() -> Result<(), SendError> {
    let Some(folder) = reference_scmdb_path() else {
        report("skipped: set P4KX_REF_SCMDB to the folder of the SCMDB dumps");
        return Ok(());
    };
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let Some(build) = archive.build() else {
        report("skipped: the Archive has no build manifest");
        return Ok(());
    };
    let Some(version) = find_dump_version(&folder, build.branch(), build.change_number())? else {
        report(&format!(
            "skipped: {} holds no SCMDB dump of {} change {}",
            folder.display(),
            build.branch(),
            build.change_number()
        ));
        return Ok(());
    };
    let dumps = Dumps {
        merged: read_dump(&folder, MERGED, &version)?,
        blueprints: read_dump(&folder, BLUEPRINTS, &version)?,
        mining: read_dump(&folder, MINING, &version)?,
    };
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let p4kx = P4kx {
        records: records_json(&forge, &dumps.record_guids())?,
        contracts: contracts(&forge)?,
    };

    let findings = compare(&dumps, &p4kx);

    report(&format!("SCMDB dumps {version}"));
    for line in findings.summary() {
        report(&line);
    }
    if !findings.is_empty() && !findings.has_mismatches() {
        report("comparison incomplete: unproven reward mappings were not compared");
    }
    assert!(
        !findings.has_mismatches(),
        "values disagree with SCMDB, e.g. {:#?}",
        findings.examples(EXAMPLES_PER_CATEGORY)
    );
    Ok(())
}

fn read_dump(folder: &Path, name: &str, version: &str) -> Result<Value, SendError> {
    let path = folder.join(dump_file_name(name, version));
    let text = std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(serde_json::from_slice(&text)?)
}

/// Returns the Record JSON of every GUID that names a Record.
fn records_json(
    forge: &DataForge<'_>,
    guids: &BTreeSet<&str>,
) -> Result<HashMap<String, Value>, SendError> {
    guids
        .par_iter()
        .filter_map(|guid| {
            let record = forge.record_by_guid(guid.parse::<Guid>().ok()?)?;
            Some(record_json(&record).map(|json| ((*guid).to_owned(), json)))
        })
        .collect()
}

/// Returns the contracts of every Record whose type or record path names contracts; of two
/// contracts with one `id`, the one in the earlier Record is kept.
fn contracts(forge: &DataForge<'_>) -> Result<HashMap<String, ContractFields>, SendError> {
    let candidates: Vec<Record<'_>> = forge.records().filter(holds_contracts).collect();
    let records: Vec<Value> = candidates
        .par_iter()
        .map(record_json)
        .collect::<Result<_, _>>()?;
    let mut contracts = HashMap::new();
    for record in &records {
        collect_contracts(record, &mut contracts);
    }
    Ok(contracts)
}

fn holds_contracts(record: &Record<'_>) -> bool {
    [record.type_name(), record.record_path()]
        .into_iter()
        .flatten()
        .any(|text| text.to_ascii_lowercase().contains(CONTRACT_MARK))
}

fn record_json(record: &Record<'_>) -> Result<Value, SendError> {
    let tree = record.tree()?;
    let mut json = Vec::new();
    RecordJson::new(JsonLayout::Compact).write(&tree, &mut json)?;
    Ok(serde_json::from_slice(&json)?)
}

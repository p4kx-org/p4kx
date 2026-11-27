// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Write as _;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ContractKey,
    ParsedCatalogQueries, ProvenanceMode,
};
use p4kx_extraction::{Archive, ArchiveWatch, DataForge, DataForgeEntry, RecordTree};
use p4kx_testkit::{real_archive_key, real_archive_path};

#[path = "contract_sdk_compare.rs"]
mod compare;
#[path = "contract_sdk_discovery.rs"]
mod discovery;
#[path = "contract_sdk_evidence.rs"]
mod evidence;
#[path = "contract_sdk_selector.rs"]
mod selector;
#[path = "contract_sdk_sets.rs"]
mod sets;
#[path = "contract_sdk_source.rs"]
mod source;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[derive(Default)]
struct Counts {
    owner_rows: usize,
    logical_rows: usize,
    set_links: usize,
    identity_cells: usize,
    reference_fields: usize,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn selected_contract_sdk_matches_original_owner_and_logical_slots() -> TestResult {
    let request = selector::Request::read()?;
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    check(&watch)?;
    let build = archive.build().ok_or("installed Build identity")?;
    request.check(build)?;
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )
    .map_err(|_| "open SDK session")?
    .with_provenance_mode(ProvenanceMode::Full);
    let parsed = session.parsed(None).map_err(|_| "parse SDK view")?;
    let forge = entry
        .open()
        .map_err(|_| "open independent original DataForge")?;
    let (selector, tree) = request.resolve(build, &forge, &watch)?;
    let counts = exercise(&archive, &entry, &parsed, &watch, &forge, &tree)?;
    check(&watch)?;
    counts.report(selector.change())
}

fn exercise<'source>(
    archive: &Archive,
    entry: &DataForgeEntry,
    parsed: &ParsedCatalogQueries<'_>,
    watch: &ArchiveWatch,
    forge: &'source DataForge<'source>,
    tree: &RecordTree<'source>,
) -> TestResult<Counts> {
    let index = tree.record_index();
    let sources = source::Rows::read(tree)?;
    let keys = sources.probe_keys()?;
    let mut counts = Counts::default();
    parsed
        .with_contracts(
            CatalogRecordSelection::RecordIndex(index),
            None,
            |snapshot| compare::owner(archive, entry, tree, &sources, &snapshot, &mut counts),
        )
        .map_err(|_| "project selected physical owner")??;
    for (contract, sub) in keys {
        check(watch)?;
        parsed
            .with_contract(ContractKey::new(contract, sub), None, |snapshot| {
                compare::logical(
                    archive,
                    entry,
                    forge,
                    (contract, sub),
                    &snapshot,
                    &mut counts,
                )
            })
            .map_err(|_| "project exact logical identity")??;
    }
    Ok(counts)
}

fn original<'source>(
    forge: &'source DataForge<'source>,
    index: u32,
) -> TestResult<RecordTree<'source>> {
    let record = forge
        .record(index)
        .ok_or("the explicit Record index is absent")?;
    if record
        .type_name()
        .map_err(|_| "read original Record type")?
        != "ContractGenerator"
    {
        return Err("the explicit Record is not a Generator".into());
    }
    record
        .tree()
        .map_err(|_| "read original selected Generator body".into())
}

fn check(watch: &ArchiveWatch) -> TestResult {
    watch
        .check()
        .map_err(|_| "Archive changed during the SDK source check".into())
}

impl Counts {
    fn report(&self, change: &str) -> TestResult {
        if self.owner_rows == 0
            || self.logical_rows != 2
            || self.set_links == 0
            || self.reference_fields == 0
        {
            return Err("selected source does not exercise owner, parent, sub-contract, direct references and set links".into());
        }
        let line = format!(
            "Contract SDK source check: change={change} owner_rows={} logical_rows={} set_links={} identity_cells={} reference_fields={}\n",
            self.owner_rows,
            self.logical_rows,
            self.set_links,
            self.identity_cells,
            self.reference_fields
        );
        if line.len() > 256 {
            return Err("aggregate witness output exceeded its byte contract".into());
        }
        std::io::stderr().lock().write_all(line.as_bytes())?;
        Ok(())
    }
}

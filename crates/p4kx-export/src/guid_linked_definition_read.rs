// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::OnceCell;

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, ProvenanceMode};

use super::lazy_loose_records::LazyLooseRecords;
use super::loose_definitions::{self, LooseDefinitionSnapshot};
use super::{CatalogLanguageSelection, CatalogQuerySession, ParsedCatalogQueries};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::loose_definition::Selection;
use crate::catalog_read::loose_record_link::{self, GuidRecordLookup};
use crate::catalog_read::original::EntryCapture;
use crate::catalog_table::CatalogTable;
use crate::export_ledger::ExportLedger;
use crate::{CatalogError, CatalogOptions, Kind};

type Project<Row> = fn(
    &Archive,
    &EntryCapture<'_>,
    &mut ExportLedger,
    &CatalogOptions<'_>,
    Selection<'_>,
    &mut GuidRecordLookup<'_>,
) -> Result<CatalogTable<Row>, CatalogError>;

pub(super) struct GuidLinkedDefinitionRead<Row> {
    kind: Kind,
    project: Project<Row>,
}

impl<Row> GuidLinkedDefinitionRead<Row> {
    pub(super) const fn new(kind: Kind, project: Project<Row>) -> Self {
        Self { kind, project }
    }

    pub(super) fn read(
        &self,
        archive: &Archive,
        selection: Selection<'_>,
        provenance_mode: ProvenanceMode,
        cancel: Option<&CancelToken>,
    ) -> Result<LooseDefinitionSnapshot<Row>, CatalogError> {
        loose_definitions::read_with_provenance_mode(
            archive,
            selection,
            cancel,
            self.kind,
            provenance_mode,
            |capture, ledger, options| {
                let entry = OnceCell::new();
                let records = LazyLooseRecords::new(archive, capture.source_uses(), cancel, &entry);
                (self.project)(
                    archive,
                    capture,
                    ledger,
                    options,
                    selection,
                    &mut |guid, selector, expected| {
                        records.with_input(|original| {
                            loose_record_link::resolve_guid(
                                guid, selector, original, expected, None, cancel,
                            )
                        })
                    },
                )
            },
        )
    }

    pub(super) fn read_from(
        &self,
        archive: &Archive,
        entry: &DataForgeEntry,
        selection: Selection<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<LooseDefinitionSnapshot<Row>, CatalogError> {
        let session = CatalogQuerySession::from_entry(
            archive,
            entry,
            CatalogLanguageSelection::Selected(&[]),
            cancel,
        )?;
        self.read_parsed(&session.parsed(cancel)?, selection, cancel)
    }

    pub(super) fn read_parsed(
        &self,
        parsed: &ParsedCatalogQueries<'_>,
        selection: Selection<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<LooseDefinitionSnapshot<Row>, CatalogError> {
        parsed.project(cancel, |archive, entry, forge| {
            loose_definitions::read_with_provenance_mode(
                archive,
                selection,
                cancel,
                self.kind,
                entry.provenance_mode(),
                |capture, ledger, options| {
                    let origins = CatalogOrigins::for_archive_records_with_uses(
                        archive,
                        entry.entry(),
                        capture.source_uses().clone(),
                    )?;
                    let original = OriginalCatalogInput::new(forge, &origins);
                    (self.project)(
                        archive,
                        capture,
                        ledger,
                        options,
                        selection,
                        &mut |guid, selector, expected| {
                            loose_record_link::resolve_guid(
                                guid, selector, &original, expected, None, cancel,
                            )
                        },
                    )
                },
            )
        })
    }
}

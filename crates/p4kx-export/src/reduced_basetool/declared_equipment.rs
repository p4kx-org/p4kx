// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, Guid, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::declared_attachment_constraints::{self as attachments, AttachmentConstraint};
use super::declared_ports::{self as ports, PortContainer};
use super::record_targets::RecordTargets;
use super::selected_texts::{SelectedTexts, TextRefs};
use crate::Kind;
use crate::catalog_queries::equipment::DeclaredEquipmentSnapshot;
use crate::catalog_queries::{CatalogRecordSelection, DatasetAvailability, ParsedCatalogQueries};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::guid_text::CatalogGuid;

pub(super) struct EquipmentTargets<'read> {
    pub(super) ships: &'read RecordTargets,
    pub(super) items: &'read RecordTargets,
}

pub(super) struct Declarations {
    pub(super) ports: StagedCatalogTable,
    pub(super) attachments: StagedCatalogTable,
}

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    targets: &EquipmentTargets<'_>,
    texts: &mut SelectedTexts,
) -> Result<Declarations, ReducedError> {
    let availability = parsed.declared_equipment_availability(cancel)?;
    let mut rows = StagedDeclarations {
        ports: StagedFamily::new(scratch, cancel),
        attachments: StagedFamily::new(scratch, cancel),
    };
    let summary =
        parsed.visit_declared_equipment(CatalogRecordSelection::All, cancel, |source| {
            rows.append(source, targets, texts, cancel)
        })?;
    completeness::inventory(ports::PATH, summary.availability(), summary.coverage())?;
    completeness::rows(ports::PATH, summary.owner_count(), summary.row_count())?;
    Ok(Declarations {
        ports: rows.ports.finish(ports::PATH, availability.ports)?,
        attachments: rows
            .attachments
            .finish(attachments::PATH, availability.attachments)?,
    })
}

struct StagedFamily<'stage> {
    rows: StagedCatalogRows<'stage>,
    has_rows: bool,
}

impl<'stage> StagedFamily<'stage> {
    const fn new(scratch: &'stage Path, cancel: Option<&'stage CancelToken>) -> Self {
        Self {
            rows: StagedCatalogRows::new(scratch, cancel),
            has_rows: false,
        }
    }

    fn append(&mut self, row: &impl Serialize) -> Result<(), ReducedError> {
        self.rows.append(row)?;
        self.has_rows = true;
        Ok(())
    }

    fn finish(
        self,
        path: &'static str,
        declared: DatasetAvailability,
    ) -> Result<StagedCatalogTable, ReducedError> {
        let availability = if self.has_rows {
            DatasetAvailability::Available
        } else {
            declared
        };
        if matches!(availability, DatasetAvailability::Unavailable) {
            return Err(ReducedError::IncompleteScope(path));
        }
        Ok(self.rows.finish(availability)?)
    }
}

struct StagedDeclarations<'stage> {
    ports: StagedFamily<'stage>,
    attachments: StagedFamily<'stage>,
}

impl StagedDeclarations<'_> {
    fn append(
        &mut self,
        snapshot: &DeclaredEquipmentSnapshot<'_>,
        targets: &EquipmentTargets<'_>,
        texts: &mut SelectedTexts,
        cancel: Option<&CancelToken>,
    ) -> Result<(), ReducedError> {
        let [source] = snapshot.rows() else {
            return Err(ReducedError::IncompleteScope(ports::PATH));
        };
        let owner = Owner {
            guid: source.source().guid(),
            record: source.source().record_index(),
            targets: match source.kind() {
                Kind::Ships => targets.ships,
                Kind::Items => targets.items,
                _ => return Err(ReducedError::IncompleteScope(ports::PATH)),
            },
        };
        let mut references = TextRefs::default();
        for container in source.port_containers() {
            self.ports
                .append(&PortContainer::read(container, &owner, &mut references)?)?;
        }
        for attachment in source.attachments() {
            self.attachments
                .append(&AttachmentConstraint::read(attachment, &owner)?)?;
        }
        texts.add(references, snapshot.public_names(), &[], cancel)
    }
}

pub(super) struct Owner<'read> {
    guid: SourcedValue<Guid>,
    record: &'read SourcedValue<u32>,
    targets: &'read RecordTargets,
}

impl Owner<'_> {
    pub(super) fn component<'read>(
        &self,
        path: &'static str,
        guid: &SourcedValue<Guid>,
        record: &'read SourcedValue<u32>,
        ordinal: &'read SourcedValue<u32>,
    ) -> Result<ComponentCoordinate<'read>, ReducedError> {
        let owner_guid = completeness::cell(path, guid)?.map(|value| CatalogGuid(*value));
        let owner_record_index = completeness::cell(path, record)?;
        let component_ordinal = completeness::cell(path, ordinal)?;
        let (Some(guid), Some(&record), Some(_)) = (
            owner_guid.value,
            owner_record_index.value,
            component_ordinal.value,
        ) else {
            return Err(ReducedError::IncompleteScope(path));
        };
        if self.guid.value() != Some(&guid.get()) || self.record.value() != Some(&record) {
            return Err(ReducedError::UnresolvedRecordLink {
                path,
                index: record,
            });
        }
        self.targets.verify(path, record, guid.get())?;
        Ok(ComponentCoordinate {
            owner_guid,
            owner_record_index,
            component_ordinal,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ComponentCoordinate<'read> {
    owner_guid: Cell<CatalogGuid>,
    owner_record_index: Cell<&'read u32>,
    component_ordinal: Cell<&'read u32>,
}

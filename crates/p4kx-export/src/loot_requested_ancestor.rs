// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, DataForge, FieldOrigin, Guid, Record, SourceUseRef, SourceUseRole,
};

use super::super::admission::{Admission, admit, record_count_source};
use super::Outcome;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_queries::loot::{
    LootAncestorSelection, LootContainerQuery, LootContainerStatus, LootDocumentAdmission,
};
use crate::catalog_query_session::checkpoint;
use crate::entity_selection::{self, Selected};
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::{CatalogError, ExportAccountingError, Filter, Kind};

#[derive(Clone, Copy)]
pub(super) enum RequestedOwner {
    NullKey,
    Record { index: u32, guid: Guid },
}

impl RequestedOwner {
    pub(super) const fn guid(self) -> Option<Guid> {
        match self {
            Self::NullKey => None,
            Self::Record { guid, .. } => Some(guid),
        }
    }
}

enum AncestorPreflight {
    Ready {
        owner: RequestedOwner,
        origin: Option<FieldOrigin>,
    },
    Excluded {
        origin: FieldOrigin,
        skipped_leftover: bool,
    },
    Unreadable {
        index: u32,
        origin: FieldOrigin,
        failed_source_use: SourceUseRef,
    },
}

fn inspect(
    forge: &DataForge<'_>,
    origins: &CatalogOrigins,
    query: LootContainerQuery<'_>,
) -> Result<AncestorPreflight, CatalogError> {
    let LootAncestorSelection::RecordIndex(index) = query.ancestor else {
        return Ok(AncestorPreflight::Ready {
            owner: RequestedOwner::NullKey,
            origin: None,
        });
    };
    let Some(record) = forge.record(index) else {
        let count = record_count_source(origins)?;
        return Ok(AncestorPreflight::Excluded {
            origin: origins.uses.origin(&[count], true),
            skipped_leftover: false,
        });
    };
    select_record(&record, origins, query.catalog_filter)
}

fn select_record(
    record: &Record<'_>,
    origins: &CatalogOrigins,
    filter: Option<&Filter>,
) -> Result<AncestorPreflight, CatalogError> {
    let mut fields = Vec::new();
    let selection = entity_selection::select_observed(record, filter, |field, role| {
        fields.push((field, role));
    });
    let index = record.index();
    let context = origins.record(index);
    let mut uses =
        vec![context.observe_header(RecordHeaderField::RecordIndex, SourceUseRole::Selector)?];
    let mut attempted = Vec::new();
    for (field, role) in fields {
        let destination = if role == SourceUseRole::Attempted {
            &mut attempted
        } else {
            &mut uses
        };
        context.append_header_input(field, role, destination)?;
    }
    match selection {
        Ok(Some(Selected::Entity(Kind::StarmapObjects))) => {
            let guid = record.guid();
            context.append_header_input(
                RecordHeaderInput::Field(RecordHeaderField::Guid),
                SourceUseRole::Selector,
                &mut uses,
            )?;
            Ok(AncestorPreflight::Ready {
                owner: RequestedOwner::Record { index, guid },
                origin: Some(origins.uses.origin(&uses, true)),
            })
        }
        Ok(excluded) => Ok(AncestorPreflight::Excluded {
            origin: origins.uses.origin(&uses, true),
            skipped_leftover: matches!(excluded, Some(Selected::Leftover(Kind::StarmapObjects))),
        }),
        Err(_) => {
            let [failed] = attempted.as_slice() else {
                return Err(ExportAccountingError::MissingFieldInspection.into());
            };
            Ok(AncestorPreflight::Unreadable {
                index,
                origin: origins.uses.origin(&uses, false),
                failed_source_use: failed.reference().clone(),
            })
        }
    }
}

impl Outcome {
    pub(super) fn admit_ancestor(
        &mut self,
        forge: &DataForge<'_>,
        origins: &CatalogOrigins,
        query: LootContainerQuery<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<Option<RequestedOwner>, CatalogError> {
        checkpoint(cancel)?;
        let owner = match inspect(forge, origins, query)? {
            AncestorPreflight::Ready { owner, origin } => {
                self.ancestor_selection_origin = origin;
                Some(owner)
            }
            AncestorPreflight::Excluded {
                origin,
                skipped_leftover,
            } => {
                if origin.source_set_complete() {
                    self.status = LootContainerStatus::NotFound;
                }
                self.records.skipped_leftovers = usize::from(skipped_leftover);
                self.ancestor_selection_origin = Some(origin);
                None
            }
            AncestorPreflight::Unreadable {
                index,
                origin,
                failed_source_use,
            } => {
                self.failed_source_uses.push(failed_source_use);
                self.records.unreadable_headers.push(index);
                self.ancestor_selection_origin = Some(origin);
                None
            }
        };
        let Some(owner) = owner else {
            return Ok(None);
        };
        let activation = admit(forge, origins, query.catalog_filter, cancel)?;
        Ok(self.admit_record_paths(activation).then_some(owner))
    }

    fn admit_record_paths(&mut self, activation: Admission) -> bool {
        self.failed_source_uses.extend(activation.failed_source_use);
        self.admission_origin = activation
            .survey
            .or_else(|| activation.record.map(|record| record.origin().clone()));
        if activation.status == LootDocumentAdmission::Absent
            && self
                .admission_origin
                .as_ref()
                .is_some_and(FieldOrigin::source_set_complete)
        {
            self.status = LootContainerStatus::NotFound;
        }
        activation.status == LootDocumentAdmission::Selected
    }
}

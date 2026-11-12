// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree};

use super::authored_fields::Observed;
use super::infraction_definition::{
    DEFINITION_TYPE, InfractionDefinition, InfractionSetDefinition, SET_TYPE,
};
use super::jurisdiction_definition::{JurisdictionDefinition, SOURCE_TYPE as JURISDICTION_TYPE};
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_table::CatalogTable;
use crate::guid_text::CatalogGuid;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError, Kind};

/// The canonical target kind of a concrete physical law Record type.
pub(crate) fn kind_for_type(source_type: &str) -> Option<Kind> {
    match source_type {
        DEFINITION_TYPE => Some(Kind::Infractions),
        SET_TYPE => Some(Kind::InfractionSets),
        JURISDICTION_TYPE => Some(Kind::Jurisdictions),
        _ => None,
    }
}

/// One physical law definition with its original Record identity and numeric receipts.
pub(crate) struct PreparedLaw<'source, T> {
    definition: T,
    guid: Guid,
    index: u32,
    path: &'source str,
    numbers: Vec<CapturedNumber>,
}

pub(crate) enum LawPick<'source> {
    Infraction(Box<PreparedLaw<'source, InfractionDefinition<'source>>>),
    Set(Box<PreparedLaw<'source, InfractionSetDefinition<'source>>>),
    Jurisdiction(Box<PreparedLaw<'source, JurisdictionDefinition<'source>>>),
}

impl<'source> LawPick<'source> {
    pub(crate) fn read(
        kind: Kind,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, Vec<FieldError>), CatalogError> {
        if kind_for_type(tree.type_name()) != Some(kind) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let input = projection.original();
        let scope = input.record_scope(admission.record(), tree)?;
        let originals = OriginalFlowFields::read(&scope, cancel)?;
        let capture = TemplateFieldCapture::new(&originals, input, admission)?;
        let numbers = RefCell::new(Vec::new());
        let fields = Observed::definition(&originals, input, &capture, &numbers);
        let mut notes = Vec::new();
        let source = CanonicalRecordHeader::read(tree, projection.origins())?;
        let picked = match kind {
            Kind::Infractions => {
                let value = InfractionDefinition::read(source, fields, projection, &mut notes)?;
                Self::Infraction(Box::new(PreparedLaw::new(
                    tree,
                    value,
                    numbers.into_inner(),
                )))
            }
            Kind::InfractionSets => {
                let value = InfractionSetDefinition::read(source, fields, projection, &mut notes)?;
                Self::Set(Box::new(PreparedLaw::new(
                    tree,
                    value,
                    numbers.into_inner(),
                )))
            }
            Kind::Jurisdictions => {
                let value = JurisdictionDefinition::read(source, fields, projection, &mut notes)?;
                Self::Jurisdiction(Box::new(PreparedLaw::new(
                    tree,
                    value,
                    numbers.into_inner(),
                )))
            }
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        };
        Ok((picked, notes))
    }
}

impl<'source, T> PreparedLaw<'source, T> {
    const fn new(tree: &RecordTree<'source>, definition: T, numbers: Vec<CapturedNumber>) -> Self {
        Self {
            definition,
            guid: tree.guid(),
            index: tree.record_index(),
            path: tree.record_path(),
            numbers,
        }
    }

    fn order(rows: &mut [Self]) {
        rows.sort_by_cached_key(|row| (CatalogGuid(row.guid).text(), row.index));
    }
}

#[derive(Default)]
pub(crate) struct LawKinds<'source> {
    infractions: Vec<PreparedLaw<'source, InfractionDefinition<'source>>>,
    sets: Vec<PreparedLaw<'source, InfractionSetDefinition<'source>>>,
    jurisdictions: Vec<PreparedLaw<'source, JurisdictionDefinition<'source>>>,
}

pub(crate) struct LawTables<'source> {
    pub(crate) infractions: CatalogTable<InfractionDefinition<'source>>,
    pub(crate) infraction_sets: CatalogTable<InfractionSetDefinition<'source>>,
    pub(crate) jurisdictions: CatalogTable<JurisdictionDefinition<'source>>,
}

impl<'source> LawKinds<'source> {
    pub(crate) fn push(&mut self, pick: LawPick<'source>) {
        match pick {
            LawPick::Infraction(row) => self.infractions.push(*row),
            LawPick::Set(row) => self.sets.push(*row),
            LawPick::Jurisdiction(row) => self.jurisdictions.push(*row),
        }
    }

    pub(crate) fn order(&mut self) {
        PreparedLaw::order(&mut self.infractions);
        PreparedLaw::order(&mut self.sets);
        PreparedLaw::order(&mut self.jurisdictions);
    }

    pub(crate) const fn count(&self, kind: Kind) -> usize {
        match kind {
            Kind::Infractions => self.infractions.len(),
            Kind::InfractionSets => self.sets.len(),
            Kind::Jurisdictions => self.jurisdictions.len(),
            _ => 0,
        }
    }

    pub(crate) fn sources(
        &self,
    ) -> impl Iterator<Item = (Kind, Guid, &'source str, &[CapturedNumber])> {
        sources(Kind::Infractions, &self.infractions)
            .chain(sources(Kind::InfractionSets, &self.sets))
            .chain(sources(Kind::Jurisdictions, &self.jurisdictions))
    }

    pub(crate) fn number_groups(&self) -> impl Iterator<Item = (Kind, &[CapturedNumber])> {
        self.sources().map(|(kind, _, _, numbers)| (kind, numbers))
    }

    pub(crate) fn into_tables(self, forge: &DataForge<'_>) -> LawTables<'source> {
        LawTables {
            infractions: table(forge, DEFINITION_TYPE, self.infractions),
            infraction_sets: table(forge, SET_TYPE, self.sets),
            jurisdictions: table(forge, JURISDICTION_TYPE, self.jurisdictions),
        }
    }
}

fn sources<'rows, 'source, T>(
    kind: Kind,
    rows: &'rows [PreparedLaw<'source, T>],
) -> impl Iterator<Item = (Kind, Guid, &'source str, &'rows [CapturedNumber])> {
    rows.iter()
        .map(move |row| (kind, row.guid, row.path, row.numbers.as_slice()))
}

fn table<T>(
    forge: &DataForge<'_>,
    source_type: &str,
    rows: Vec<PreparedLaw<'_, T>>,
) -> CatalogTable<T> {
    CatalogTable::new(
        forge,
        &[source_type],
        rows.into_iter().map(|row| row.definition).collect(),
    )
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree};

use super::infraction_definition::{
    DEFINITION_TYPE, InfractionDefinition, InfractionSetDefinition, SET_TYPE,
};
use super::jurisdiction_definition::{JurisdictionDefinition, SOURCE_TYPE as JURISDICTION_TYPE};
use super::prepared_definition::PreparedDefinition;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
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

pub(crate) enum LawPick<'source> {
    Infraction(Box<PreparedDefinition<'source, InfractionDefinition<'source>>>),
    Set(Box<PreparedDefinition<'source, InfractionSetDefinition<'source>>>),
    Jurisdiction(Box<PreparedDefinition<'source, JurisdictionDefinition<'source>>>),
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
        match kind {
            Kind::Infractions => PreparedDefinition::read(
                tree,
                projection,
                admission,
                cancel,
                InfractionDefinition::read,
            )
            .map(|(row, notes)| (Self::Infraction(Box::new(row)), notes)),
            Kind::InfractionSets => PreparedDefinition::read(
                tree,
                projection,
                admission,
                cancel,
                InfractionSetDefinition::read,
            )
            .map(|(row, notes)| (Self::Set(Box::new(row)), notes)),
            Kind::Jurisdictions => PreparedDefinition::read(
                tree,
                projection,
                admission,
                cancel,
                JurisdictionDefinition::read,
            )
            .map(|(row, notes)| (Self::Jurisdiction(Box::new(row)), notes)),
            _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
    }
}

#[derive(Default)]
pub(crate) struct LawKinds<'source> {
    infractions: Vec<PreparedDefinition<'source, InfractionDefinition<'source>>>,
    sets: Vec<PreparedDefinition<'source, InfractionSetDefinition<'source>>>,
    jurisdictions: Vec<PreparedDefinition<'source, JurisdictionDefinition<'source>>>,
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
        PreparedDefinition::order(&mut self.infractions);
        PreparedDefinition::order(&mut self.sets);
        PreparedDefinition::order(&mut self.jurisdictions);
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
        PreparedDefinition::sources(Kind::Infractions, &self.infractions)
            .chain(PreparedDefinition::sources(
                Kind::InfractionSets,
                &self.sets,
            ))
            .chain(PreparedDefinition::sources(
                Kind::Jurisdictions,
                &self.jurisdictions,
            ))
    }

    pub(crate) fn number_groups(&self) -> impl Iterator<Item = (Kind, &[CapturedNumber])> {
        self.sources().map(|(kind, _, _, numbers)| (kind, numbers))
    }

    pub(crate) fn into_tables(self, forge: &DataForge<'_>) -> LawTables<'source> {
        LawTables {
            infractions: PreparedDefinition::table(forge, DEFINITION_TYPE, self.infractions),
            infraction_sets: PreparedDefinition::table(forge, SET_TYPE, self.sets),
            jurisdictions: PreparedDefinition::table(forge, JURISDICTION_TYPE, self.jurisdictions),
        }
    }
}

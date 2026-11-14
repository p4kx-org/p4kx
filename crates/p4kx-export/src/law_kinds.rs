// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree};
use schemars::JsonSchema;
use serde::Serialize;

use self::security_network_manifest::SecurityNetworkManifest;
use super::infraction_definition::{
    DEFINITION_TYPE, InfractionDefinition, InfractionSetDefinition, SET_TYPE,
};
use super::jurisdiction_definition::{JurisdictionDefinition, SOURCE_TYPE as JURISDICTION_TYPE};
use super::prepared_definition::PreparedDefinition;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError, Kind};

#[path = "security_network_manifest.rs"]
mod security_network_manifest;

pub(crate) const SECURITY_NETWORK_TYPE: &str = security_network_manifest::SOURCE_TYPE;

/// The canonical target kind of a concrete physical law Record type.
pub(crate) fn kind_for_type(source_type: &str) -> Option<Kind> {
    match source_type {
        DEFINITION_TYPE => Some(Kind::Infractions),
        SET_TYPE => Some(Kind::InfractionSets),
        JURISDICTION_TYPE => Some(Kind::Jurisdictions),
        SECURITY_NETWORK_TYPE => Some(Kind::SecurityNetworkManifests),
        _ => None,
    }
}

pub(crate) enum LawPick<'source> {
    Infraction(Box<PreparedDefinition<'source, InfractionDefinition<'source>>>),
    Set(Box<PreparedDefinition<'source, InfractionSetDefinition<'source>>>),
    Jurisdiction(Box<PreparedDefinition<'source, JurisdictionDefinition<'source>>>),
    SecurityNetwork(Box<PreparedDefinition<'source, SecurityNetworkManifest<'source>>>),
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
            Kind::SecurityNetworkManifests => PreparedDefinition::read(
                tree,
                projection,
                admission,
                cancel,
                SecurityNetworkManifest::read,
            )
            .map(|(row, notes)| (Self::SecurityNetwork(Box::new(row)), notes)),
            _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
    }
}

#[derive(Default)]
pub(crate) struct LawKinds<'source> {
    infractions: Vec<PreparedDefinition<'source, InfractionDefinition<'source>>>,
    sets: Vec<PreparedDefinition<'source, InfractionSetDefinition<'source>>>,
    jurisdictions: Vec<PreparedDefinition<'source, JurisdictionDefinition<'source>>>,
    security_networks: Vec<PreparedDefinition<'source, SecurityNetworkManifest<'source>>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LawTables<'source> {
    infractions: CatalogTable<InfractionDefinition<'source>>,
    infraction_sets: CatalogTable<InfractionSetDefinition<'source>>,
    jurisdictions: CatalogTable<JurisdictionDefinition<'source>>,
    security_network_manifests: CatalogTable<SecurityNetworkManifest<'source>>,
}

impl<'source> LawKinds<'source> {
    pub(crate) fn push(&mut self, pick: LawPick<'source>) {
        match pick {
            LawPick::Infraction(row) => self.infractions.push(*row),
            LawPick::Set(row) => self.sets.push(*row),
            LawPick::Jurisdiction(row) => self.jurisdictions.push(*row),
            LawPick::SecurityNetwork(row) => self.security_networks.push(*row),
        }
    }

    pub(crate) fn order(&mut self) {
        PreparedDefinition::order(&mut self.infractions);
        PreparedDefinition::order(&mut self.sets);
        PreparedDefinition::order(&mut self.jurisdictions);
        PreparedDefinition::order(&mut self.security_networks);
    }

    pub(crate) const fn count(&self, kind: Kind) -> usize {
        match kind {
            Kind::Infractions => self.infractions.len(),
            Kind::InfractionSets => self.sets.len(),
            Kind::Jurisdictions => self.jurisdictions.len(),
            Kind::SecurityNetworkManifests => self.security_networks.len(),
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
            .chain(PreparedDefinition::sources(
                Kind::SecurityNetworkManifests,
                &self.security_networks,
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
            security_network_manifests: PreparedDefinition::table(
                forge,
                SECURITY_NETWORK_TYPE,
                self.security_networks,
            ),
        }
    }
}

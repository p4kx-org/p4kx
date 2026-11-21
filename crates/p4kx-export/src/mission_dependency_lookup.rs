// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, SourceUse, SourceValue};

use super::demand::{DependencyKey, PreparedDependencies};
use super::read::selection::Action;
use super::{Broker, MissionIndex, Template};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::ProtectedKey;
use crate::catalog_read::original::OriginalReferenceObservation;
use crate::mission_text_inputs::{MissionItemTexts, OrganizationTexts};
use crate::record_read::ReadError;

impl<'source> MissionIndex<'source> {
    pub(crate) fn set_query_dependencies(&mut self, prepared: PreparedDependencies) {
        self.prepared = prepared;
    }

    pub(crate) fn query_dependencies(&self) -> PreparedDependencies {
        self.prepared.clone()
    }

    pub(crate) fn lookup_template(
        &self,
        guid: Guid,
    ) -> Result<Option<&Template<'source>>, ReadError> {
        self.prepared
            .require(DependencyKey::Mission(Action::Template, guid))?;
        Ok(self.template(guid))
    }

    pub(crate) fn lookup_broker(&self, guid: Guid) -> Result<Option<&Broker<'source>>, ReadError> {
        self.prepared
            .require(DependencyKey::Mission(Action::Broker, guid))?;
        Ok(self.broker(guid))
    }

    pub(crate) fn lookup_mission_type_name(
        &self,
        guid: Guid,
    ) -> Result<Option<&ProtectedKey<'source>>, ReadError> {
        self.prepared
            .require(DependencyKey::Mission(Action::Type, guid))?;
        Ok(self.mission_type_name(guid))
    }

    pub(crate) fn lookup_faction_name(
        &self,
        guid: Guid,
    ) -> Result<Option<&ProtectedKey<'source>>, ReadError> {
        self.prepared
            .require(DependencyKey::Mission(Action::Faction, guid))?;
        Ok(self.faction_name(guid))
    }

    pub(crate) fn lookup_locality(
        &self,
        guid: Guid,
        record_index: u32,
    ) -> Result<&[Guid], ReadError> {
        self.prepared
            .require(DependencyKey::MissionLocality { guid, record_index })?;
        Ok(self.locality(guid, record_index))
    }

    pub(crate) fn lookup_item(
        &self,
        guid: Guid,
        record_index: u32,
    ) -> Result<Option<&MissionItemTexts<'source>>, ReadError> {
        self.prepared
            .require(DependencyKey::MissionItem { guid, record_index })?;
        Ok(self.item(guid, record_index))
    }

    pub(crate) fn lookup_organization(
        &self,
        guid: Guid,
    ) -> Result<Option<&OrganizationTexts<'source>>, ReadError> {
        self.prepared
            .require(DependencyKey::Mission(Action::Organization, guid))?;
        Ok(self.organization(guid))
    }

    pub(crate) fn lookup_nominal_reward_amount(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
        reference: &OriginalReferenceObservation<'_, '_, '_, '_>,
        uses: &mut Vec<SourceUse>,
    ) -> Result<SourceValue<i64>, ReadError> {
        self.prepared.require(DependencyKey::Mission(
            Action::Reward,
            reference.reference().guid(),
        ))?;
        Ok(self.nominal_reward_amount(input, reference, uses)?)
    }
}

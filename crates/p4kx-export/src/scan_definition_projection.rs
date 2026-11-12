// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::RecordTree;

use super::Scan;
use crate::Kind;
use crate::loot_kinds::LootPick;
use crate::picked::Picked;
use crate::record_observation::RecordObservation;
use crate::scan_health::ScanError;
use crate::source_fields::FieldError;

impl<'f> Scan<'_, '_, 'f> {
    pub(super) fn read_definition(
        &self,
        kind: Kind,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
    ) -> Result<Option<(Picked<'f>, Vec<FieldError>)>, ScanError> {
        let projection = match kind {
            Kind::LootPoolFilters => {
                let value = crate::catalog_v3::loot_filter_catalog::read(
                    admission.record(),
                    tree,
                    self.projection,
                    self.options.cancel,
                )?;
                (Picked::Loot(LootPick::Filter(Box::new(value))), Vec::new())
            }
            Kind::ExplosionDefinitions => self.explosion_definition(admission, tree)?,
            Kind::Factions => self.faction_definition(admission, tree)?,
            Kind::FactionReputations => self.faction_reputation_definition(admission, tree)?,
            Kind::ReputationContexts => self.context_definition(admission, tree)?,
            Kind::ReputationScopes => self.scope_definition(admission, tree)?,
            Kind::ReputationStandings => self.standing_definition(admission, tree)?,
            Kind::ReputationPerks => self.perk_definition(admission, tree)?,
            Kind::BlueprintCategoryLists => {
                let read = crate::catalog_v3::blueprint_category_list_read::read(
                    tree,
                    self.projection,
                    admission,
                    self.options.cancel,
                )?;
                (Picked::BlueprintCategoryList(read.value), read.notes)
            }
            Kind::BlueprintCategories => {
                let read = crate::blueprint_category::read(
                    tree,
                    self.projection,
                    admission,
                    self.options.cancel,
                )?;
                (Picked::BlueprintCategory(read.value), read.notes)
            }
            Kind::Manufacturers => {
                let read = crate::manufacturer::read(
                    tree,
                    self.projection,
                    admission,
                    self.options.cancel,
                )?;
                (Picked::Manufacturer(Box::new(read.value)), read.notes)
            }
            Kind::ResourceTypes => {
                let read = crate::resource_read::read(
                    tree,
                    self.projection,
                    self.groups.all_of_resource(&tree.guid()),
                    admission,
                    self.options.cancel,
                )?;
                (Picked::ResourceType(Box::new(read.value)), read.notes)
            }
            Kind::ResourceGroups => self.resource_group(admission, tree)?,
            _ => return Ok(None),
        };
        Ok(Some(projection))
    }

    fn explosion_definition(
        &self,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
    ) -> Result<(Picked<'f>, Vec<FieldError>), ScanError> {
        let read = crate::catalog_v3::explosion_definition::read(
            tree,
            self.projection,
            admission,
            self.options.cancel,
        )?;
        Ok((
            Picked::ExplosionDefinition(Box::new(read.value)),
            read.notes,
        ))
    }

    fn faction_definition(
        &self,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
    ) -> Result<(Picked<'f>, Vec<FieldError>), ScanError> {
        let read = crate::catalog_v3::faction_definition::read(
            tree,
            self.projection,
            admission,
            self.options.cancel,
        )?;
        Ok((Picked::Faction(Box::new(read.value)), read.notes))
    }

    fn context_definition(
        &self,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
    ) -> Result<(Picked<'f>, Vec<FieldError>), ScanError> {
        let read = crate::catalog_v3::reputation_context_definition::read(
            tree,
            self.projection,
            admission,
            self.options.cancel,
        )?;
        let picked = crate::mission_kinds::MissionPick::Context(Box::new(read.value));
        Ok((Picked::Mission(picked), read.notes))
    }

    fn scope_definition(
        &self,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
    ) -> Result<(Picked<'f>, Vec<FieldError>), ScanError> {
        let read = crate::catalog_v3::reputation_scope_definition::read(
            tree,
            self.projection,
            admission,
            self.options.cancel,
        )?;
        let picked = crate::mission_kinds::MissionPick::Scope(Box::new(read.value));
        Ok((Picked::Mission(picked), read.notes))
    }

    fn standing_definition(
        &self,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
    ) -> Result<(Picked<'f>, Vec<FieldError>), ScanError> {
        let read = crate::catalog_v3::reputation_standing_definition::read(
            tree,
            self.projection,
            admission,
            self.options.cancel,
        )?;
        let picked = crate::mission_kinds::MissionPick::Standing(Box::new(read.value));
        Ok((Picked::Mission(picked), read.notes))
    }

    fn perk_definition(
        &self,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
    ) -> Result<(Picked<'f>, Vec<FieldError>), ScanError> {
        let read = crate::catalog_v3::reputation_perk_list::read(
            tree,
            self.projection,
            admission,
            self.options.cancel,
        )?;
        let picked = crate::mission_kinds::MissionPick::Perks(Box::new(read.value));
        Ok((Picked::Mission(picked), read.notes))
    }

    fn faction_reputation_definition(
        &self,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
    ) -> Result<(Picked<'f>, Vec<FieldError>), ScanError> {
        let read = crate::catalog_v3::reputation_faction_definition::read(
            tree,
            self.projection,
            admission,
            self.options.cancel,
        )?;
        let picked = crate::mission_kinds::MissionPick::Faction(Box::new(read.value));
        Ok((Picked::Mission(picked), read.notes))
    }

    fn resource_group(
        &self,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
    ) -> Result<(Picked<'f>, Vec<FieldError>), ScanError> {
        let parent = self
            .groups
            .parent_choice(&tree.guid(), self.options.cancel)?;
        let read = crate::resource_group_read::read(
            tree,
            self.projection,
            parent,
            admission,
            self.options.cancel,
        )?;
        Ok((Picked::ResourceGroup(read.value), read.notes))
    }
}

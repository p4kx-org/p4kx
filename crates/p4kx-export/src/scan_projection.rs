// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Record, RecordTree, TreeBuilder};

use super::Scan;
use crate::catalog_v3::catalog_item_components::{self, ItemComponentsRead, resolution};
use crate::commodity_resource::{self, CommodityResource};
use crate::contract_stats::ItemStats;
use crate::export_diagnostic::{ExportDiagnosticCode, ExportSourceRole};
use crate::export_ledger::{DerivedFailure, ExportLedger, SourceDiagnosis};
use crate::harvest_config::multi_config;
use crate::harvest_preset::provider_preset;
use crate::loot_kinds::LootPick;
use crate::mining::mineable_entity;
use crate::picked::Picked;
use crate::record_observation::RecordObservation;
use crate::scan_health::ScanError;
use crate::source_fields::FieldError;
use crate::starmap::starmap_object;
use crate::{CatalogError, ExportAccountingError, Kind, item_stats, item_variant};

struct ResourceRead<'source> {
    record: Record<'source>,
    tree: RecordTree<'source>,
}

impl<'f> Scan<'_, '_, 'f> {
    pub(super) fn project(
        &self,
        kind: Kind,
        trees: &mut TreeBuilder<'f>,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Option<Picked<'f>>, ScanError> {
        if let Some(picked) = self.definition_kind(kind, admission, tree, ledger)? {
            return Ok(Some(picked));
        }
        if let Some(picked) = self.harvestable(kind, trees, tree) {
            return Ok(Some(picked));
        }
        if kind == Kind::MineableEntities {
            let entity = Picked::MineableEntity(mineable_entity(tree, self.signature_slot));
            return Ok(Some(entity.crafted(tree)));
        }
        if kind == Kind::StarmapObjects {
            let object = starmap_object(
                trees,
                tree,
                self.texts,
                self.forge,
                &self.types,
                self.placements,
            );
            return Ok(Some(Picked::Starmap(object)));
        }
        if kind == Kind::Ships {
            return Ok(Some(self.ship(admission, tree, ledger)?.crafted(tree)));
        }
        if kind == Kind::Commodities {
            return Ok(Some(
                self.commodity(trees, admission, tree, ledger)?
                    .crafted(tree),
            ));
        }
        let mut picked = Picked::entity(kind, tree, self.texts);
        if let Some(Picked::Item {
            entity, components, ..
        }) = &mut picked
        {
            let (read, stats) = self.item_components(tree, trees, admission, ledger)?;
            *components = read.rows;
            entity.variant_key = item_variant::key(tree, stats.as_ref());
            entity.stats = stats.and_then(ItemStats::into_published).map(Box::new);
        }
        Ok(picked.map(|picked| picked.crafted(tree)))
    }

    fn item_components(
        &self,
        tree: &RecordTree<'f>,
        trees: &mut TreeBuilder<'f>,
        admission: &RecordObservation<'_, 'f>,
        ledger: &mut ExportLedger,
    ) -> Result<(ItemComponentsRead<'f>, Option<ItemStats<'f>>), CatalogError> {
        let cancel = self.options.cancel;
        catalog_item_components::read(
            tree,
            self.projection,
            admission,
            cancel,
            |originals, read| {
                crate::source_projection_notes::record(
                    Kind::Items,
                    ExportSourceRole::Selected,
                    admission,
                    tree,
                    std::mem::take(&mut read.notes),
                    ledger,
                )?;
                let health = read.legacy_health;
                item_stats::read_with_sources(
                    tree,
                    self.forge,
                    trees,
                    || health,
                    |record, _| self.ammo.dependency(record, ledger),
                    |observed| {
                        let fields =
                            originals.ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                        if let Some(row) = resolution::read(
                            self.projection.original(),
                            admission.record(),
                            tree,
                            fields,
                            observed,
                            cancel,
                        )? {
                            read.rows.magazine_resolutions.push(row);
                        }
                        Ok(())
                    },
                )
            },
        )
    }

    fn definition_kind(
        &self,
        kind: Kind,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Option<Picked<'f>>, ScanError> {
        let Some((picked, notes)) = self.read_definition(kind, admission, tree)? else {
            return Ok(None);
        };
        crate::source_projection_notes::record(
            kind,
            ExportSourceRole::Selected,
            admission,
            tree,
            notes,
            ledger,
        )?;
        Ok(Some(picked))
    }

    fn read_definition(
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

    fn ship(
        &self,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Picked<'f>, CatalogError> {
        let record = admission.record();
        let read = self.ships.read_catalog(
            record,
            tree,
            ledger,
            self.projection.original(),
            self.options.cancel,
        )?;
        ledger.merge(read.ledger);
        let mut picked = Picked::of_ship(read.read, |read| {
            crate::ship::ship(tree, self.texts, &self.evidence, read)
        });
        if let Picked::Ship(_, vehicle) = &mut picked {
            vehicle.source_record_index = Some(record.index());
        }
        Ok(picked)
    }

    /// Returns the provider preset or multi config `tree` describes when `kind` is one of the
    /// kinds that follow a harvestable to its entity.
    fn harvestable(
        &self,
        kind: Kind,
        trees: &mut TreeBuilder<'f>,
        tree: &RecordTree<'f>,
    ) -> Option<Picked<'f>> {
        match kind {
            Kind::HarvestableProviderPresets => Some(Picked::Provider(provider_preset(
                trees,
                tree,
                self.forge,
                &self.links,
            ))),
            Kind::SubHarvestableMultiConfigs => Some(Picked::MultiConfig(multi_config(
                trees,
                tree,
                self.forge,
                &self.links,
            ))),
            _ => None,
        }
    }

    fn commodity(
        &self,
        trees: &mut TreeBuilder<'f>,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Picked<'f>, ScanError> {
        let selected = commodity_resource::first(tree)?;
        let resource_type = selected
            .link
            .as_ref()
            .map(|link| self.resource_type(trees, admission, tree, link, ledger))
            .transpose()?
            .flatten();
        let read = crate::commodity::commodity(
            tree,
            self.projection,
            resource_type.as_ref().map(|resource| &resource.tree),
            &self.groups,
            &selected,
        )?;
        crate::source_projection_notes::record(
            Kind::Commodities,
            ExportSourceRole::Selected,
            admission,
            tree,
            read.notes,
            ledger,
        )?;
        if let Some(resource) = &resource_type {
            crate::source_projection_notes::record(
                Kind::Commodities,
                ExportSourceRole::Dependency,
                &RecordObservation::capture(self.projection.original(), &resource.record)?,
                &resource.tree,
                read.resource_notes,
                ledger,
            )?;
        }
        Ok(Picked::Commodity(Box::new(read.value)))
    }
    fn resource_type(
        &self,
        trees: &mut TreeBuilder<'f>,
        commodity: &RecordObservation<'_, 'f>,
        commodity_tree: &RecordTree<'f>,
        link: &CommodityResource<'_, 'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Option<ResourceRead<'f>>, ScanError> {
        if link.reference.target_type.as_deref() != Some("ResourceType") {
            return Ok(None);
        }
        let Some(index) = link.reference.target_record_index else {
            return Ok(None);
        };
        let record = self.forge.record(index).ok_or(CatalogError::Accounting(
            ExportAccountingError::MissingPhysicalReferenceTarget { index },
        ))?;
        match trees.build(&record) {
            Ok(tree) => {
                self.projection.original().record_scope(&record, &tree)?;
                Ok(Some(ResourceRead { record, tree }))
            }
            Err(error) => {
                let failure = crate::export_source_failure::diagnosis(&error);
                let body = link.failure_body(
                    self.projection.original(),
                    commodity,
                    commodity_tree,
                    self.options.cancel,
                )?;
                let diagnosis = SourceDiagnosis::from_body(
                    failure
                        .code
                        .unwrap_or(ExportDiagnosticCode::SourceReadFailure),
                    body,
                )?;
                ledger.derived_failure(
                    DerivedFailure::of_subject(
                        commodity.subject().clone(),
                        Kind::Commodities,
                        Kind::Commodities,
                        ExportSourceRole::Selected,
                        diagnosis,
                    )
                    .map_err(CatalogError::from)?,
                );
                Ok(None)
            }
        }
    }
}

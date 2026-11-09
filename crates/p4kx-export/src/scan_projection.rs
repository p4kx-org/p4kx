// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Record, RecordTree, TreeBuilder};

use super::Scan;
use crate::commodity_resource::{self, CommodityResource};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::harvest_config::multi_config;
use crate::harvest_preset::provider_preset;
use crate::mining::mineable_entity;
use crate::picked::Picked;
use crate::scan_health::ScanError;
use crate::starmap::starmap_object;
use crate::{CatalogError, Kind, item_stats, item_variant};

impl<'f> Scan<'_, '_, 'f> {
    pub(super) fn project(
        &self,
        kind: Kind,
        trees: &mut TreeBuilder<'f>,
        record: &Record<'f>,
        tree: &RecordTree<'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Option<Picked<'f>>, ScanError> {
        if let Some(picked) = self.definition_kind(kind, tree, ledger)? {
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
            return Ok(Some(self.ship(record, tree, ledger)?.crafted(tree)));
        }
        if kind == Kind::Commodities {
            return Ok(Some(self.commodity(trees, tree, ledger)?.crafted(tree)));
        }
        let mut picked = Picked::entity(kind, tree, self.texts);
        if let Some(Picked::Item(item, _, _)) = &mut picked {
            item.stats = item_stats::read(tree, self.forge, trees).map(Box::new);
            item.variant_key = item_variant::key(tree, item.stats.as_deref());
        }
        Ok(picked.map(|picked| picked.crafted(tree)))
    }

    fn definition_kind(
        &self,
        kind: Kind,
        tree: &RecordTree<'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Option<Picked<'f>>, ScanError> {
        let (picked, notes) = match kind {
            Kind::Manufacturers => {
                let read = crate::manufacturer::read(tree, self.projection)?;
                (Picked::Manufacturer(Box::new(read.value)), read.notes)
            }
            Kind::ResourceTypes => {
                let read = crate::resource_read::read(
                    tree,
                    self.projection,
                    self.groups.all_of_resource(&tree.guid()),
                )?;
                (Picked::ResourceType(Box::new(read.value)), read.notes)
            }
            Kind::ResourceGroups => {
                let read = crate::resource_group_read::read(
                    tree,
                    self.texts,
                    self.groups.parent_of(&tree.guid()),
                )?;
                (Picked::ResourceGroup(read.value), read.notes)
            }
            _ => return Ok(None),
        };
        crate::source_projection_notes::record(
            kind,
            ExportSourceRole::Selected,
            tree,
            notes,
            ledger,
        );
        Ok(Some(picked))
    }

    fn ship(
        &self,
        record: &Record<'f>,
        tree: &RecordTree<'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Picked<'f>, CatalogError> {
        let read = self
            .ships
            .read_catalog(record, tree, ledger, self.options.cancel)?;
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
        tree: &RecordTree<'f>,
        ledger: &mut ExportLedger,
    ) -> Result<Picked<'f>, ScanError> {
        let selected = commodity_resource::first(tree)?;
        let resource_type = selected
            .link
            .as_ref()
            .map(|link| self.resource_type(trees, tree, link, ledger))
            .transpose()?
            .flatten();
        let read = crate::commodity::commodity(
            tree,
            self.projection,
            resource_type.as_ref(),
            &self.groups,
            &selected,
        )?;
        crate::source_projection_notes::record(
            Kind::Commodities,
            ExportSourceRole::Selected,
            tree,
            read.notes,
            ledger,
        );
        if let Some(resource) = &resource_type {
            crate::source_projection_notes::record(
                Kind::Commodities,
                ExportSourceRole::Dependency,
                resource,
                read.resource_notes,
                ledger,
            );
        }
        Ok(Picked::Commodity(Box::new(read.value)))
    }
    fn resource_type(
        &self,
        trees: &mut TreeBuilder<'f>,
        commodity: &RecordTree<'f>,
        link: &CommodityResource,
        ledger: &mut ExportLedger,
    ) -> Result<Option<RecordTree<'f>>, ScanError> {
        if link.reference.target_type.as_deref() != Some("ResourceType") {
            return Ok(None);
        }
        let Some(index) = link.reference.target_record_index else {
            return Ok(None);
        };
        let record = self.forge.record(index).ok_or(CatalogError::Accounting(
            crate::ExportAccountingError::MissingPhysicalReferenceTarget { index },
        ))?;
        match trees.build(&record) {
            Ok(tree) => Ok(Some(tree)),
            Err(error) => {
                let failure = crate::export_source_failure::diagnosis(&error);
                ledger.derived_failure(crate::export_ledger::DerivedFailure {
                    affected_kind: Kind::Commodities,
                    output_kind: Kind::Commodities,
                    record_index: commodity.record_index(),
                    role: ExportSourceRole::Selected,
                    guid: commodity.guid(),
                    record_path: Some(commodity.record_path().to_owned()),
                    diagnosis: crate::export_ledger::SourceDiagnosis {
                        code: failure
                            .code
                            .or(Some(crate::ExportDiagnosticCode::SourceReadFailure)),
                        field: Some(link.field.clone()),
                        source_type: Some(link.source_type.clone()),
                        target_guid: Some(link.reference.guid.0),
                    },
                });
                Ok(None)
            }
        }
    }
}

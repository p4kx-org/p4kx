// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, SourceUses, SourceValue, SourcedValue};

use crate::contract_shops::ShopItemGuids;
use crate::shop_containers::Inventory;
use crate::shop_inventories::{ItemList, is_placeholder};
use crate::{CatalogError, ExportAccountingError};

use super::{InventoryResolver, ResolutionStatus, ShopInventoryResolution};

pub(crate) struct ShopInventoryCells {
    pub(crate) inventory: SourcedValue<String>,
    pub(crate) item_guids: SourcedValue<ShopItemGuids>,
}

#[derive(Clone, Copy)]
enum Unavailable {
    Absent,
    Unreadable,
    Ambiguous,
}

impl Unavailable {
    const fn value<T>(self) -> SourceValue<T> {
        match self {
            Self::Absent => SourceValue::Absent,
            Self::Unreadable => SourceValue::Unreadable,
            Self::Ambiguous => SourceValue::Invalid,
        }
    }

    fn cells(
        self,
        origin: &FieldOrigin,
        sources: &SourceUses,
    ) -> Result<ShopInventoryCells, CatalogError> {
        Ok(ShopInventoryCells {
            inventory: sources.value_with_origins(self.value(), &[origin])?,
            item_guids: sources.value_with_origins(self.value(), &[origin])?,
        })
    }
}

#[cfg(test)]
impl ShopInventoryCells {
    pub(crate) fn unscanned(sources: &SourceUses) -> Self {
        Self {
            inventory: sources.value(SourceValue::Unreadable, &[], false),
            item_guids: sources.value(SourceValue::Unreadable, &[], false),
        }
    }
}

impl InventoryResolver<'_, '_> {
    pub(crate) fn compatibility(
        &self,
        resolution: &ShopInventoryResolution,
        entity_guid: &str,
    ) -> Result<ShopInventoryCells, CatalogError> {
        let sources = self.capture.source_uses();
        let basis = resolution.candidates.origin();
        match self.containers.inventory(entity_guid) {
            Inventory::One(stem) if !is_placeholder(stem) => self.compatible_file(stem, basis),
            Inventory::One(_) => Unavailable::Absent.cells(basis, sources),
            Inventory::Ambiguous => Unavailable::Ambiguous.cells(basis, sources),
            Inventory::None
                if resolution.status.value() == Some(&ResolutionStatus::NoDeclaredFile) =>
            {
                Unavailable::Absent.cells(basis, sources)
            }
            Inventory::None | Inventory::Unknown => {
                let origin = sources.origin_with_origins([basis], false)?;
                Unavailable::Unreadable.cells(&origin, sources)
            }
        }
    }

    fn compatible_file(
        &self,
        stem: &str,
        basis: &FieldOrigin,
    ) -> Result<ShopInventoryCells, CatalogError> {
        let sources = self.capture.source_uses();
        let target = self.inventories.files(stem).last();
        let selected = target
            .map(|file| {
                file.reference(self.capture)
                    .map(|datum| datum.into_cell(sources))
            })
            .transpose()?;
        let directory = self.inventories.directory_origin();
        let origins = [
            Some(basis),
            Some(directory),
            selected.as_ref().map(SourcedValue::origin),
        ];
        let found = self
            .lists
            .get(stem)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if let ItemList::Items { guids, .. } = found {
            let origin = sources.origin_with_origins(origins.into_iter().flatten(), true)?;
            let item_guids = sources.add_value_origins(guids.clone(), &[&origin])?;
            return Ok(ShopInventoryCells {
                inventory: item_guids.map_ref(|_| stem.to_owned()),
                item_guids,
            });
        }
        let payload = target.and_then(|file| file.content.as_ref());
        let observed = payload.map(|content| sources.origin(std::slice::from_ref(content), true));
        let unavailable = match found {
            ItemList::EmptySource(_) => Unavailable::Absent,
            ItemList::Missing if directory.source_set_complete() => Unavailable::Absent,
            _ => Unavailable::Unreadable,
        };
        let origin = sources.origin_with_origins(
            origins.into_iter().flatten().chain(observed.as_ref()),
            !matches!(unavailable, Unavailable::Unreadable),
        )?;
        unavailable.cells(&origin, sources)
    }
}

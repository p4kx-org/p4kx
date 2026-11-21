// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, SourceUse, SourceUseRole, Value};

use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::catalog_v3::catalog_facility::FacilityKind;
use crate::catalog_v3::inventory_owner_storage;
use crate::{CatalogError, ExportAccountingError};

pub(in crate::catalog_v3) struct OwnerComponents {
    pub(super) record: u32,
    pub(super) storage: StorageSelection,
    pub(super) origin: FieldOrigin,
}

#[derive(Clone, Copy)]
pub(super) enum StorageSelection {
    Unique(usize),
    Missing,
    Ambiguous,
    Unreadable,
}

impl OwnerComponents {
    pub(in crate::catalog_v3) fn read(
        record: u32,
        originals: &OriginalFlowFields<'_, '_, '_>,
        components: &Value<'_>,
    ) -> Result<Self, CatalogError> {
        let fact = originals.fact(components)?;
        originals.span(fact)?;
        let Value::Array(slots) = fact.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let mut selectors = vec![fact.observe(OriginalValuePart::Value, SourceUseRole::Selector)?];
        let storage = Self::select_storage(originals, slots, &mut selectors)?;
        let origin = originals
            .sources()
            .origin(&selectors, !matches!(storage, StorageSelection::Unreadable));
        Ok(Self {
            record,
            storage,
            origin,
        })
    }

    fn select_storage(
        originals: &OriginalFlowFields<'_, '_, '_>,
        slots: &[Value<'_>],
        selectors: &mut Vec<SourceUse>,
    ) -> Result<StorageSelection, CatalogError> {
        let mut grid = false;
        let mut storage = None;
        let mut unreadable = false;
        for (ordinal, slot) in slots.iter().enumerate() {
            originals.check_cancel()?;
            let fact = originals.fact(slot)?;
            match slot {
                Value::Struct(component) => {
                    selectors.push(
                        fact.observe(OriginalValuePart::ConcreteType, SourceUseRole::Selector)?,
                    );
                    let source_type = component.type_name();
                    if source_type == inventory_owner_storage::ITEM_COMPONENT {
                        if storage.replace(ordinal).is_some() {
                            return Ok(StorageSelection::Ambiguous);
                        }
                    } else if matches!(FacilityKind::of(source_type), Some(FacilityKind::CargoGrid))
                    {
                        if grid {
                            return Ok(StorageSelection::Ambiguous);
                        }
                        grid = true;
                    }
                }
                Value::Null => {
                    selectors
                        .push(fact.observe(OriginalValuePart::Value, SourceUseRole::Selector)?);
                }
                _ => {
                    selectors
                        .push(fact.observe(OriginalValuePart::Value, SourceUseRole::Attempted)?);
                    unreadable = true;
                }
            }
        }
        if unreadable {
            return Ok(StorageSelection::Unreadable);
        }
        Ok(storage.map_or(StorageSelection::Missing, StorageSelection::Unique))
    }
}

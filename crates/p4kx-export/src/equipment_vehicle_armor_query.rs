// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::EquipmentSnapshot;
use super::{read, rows};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

pub use crate::catalog_v3::catalog_entity_configurations::{
    ArmorDamageInfo, ArmorDamageSource, ArmorDeflection, ArmorHealthCurve,
    ArmorPenetrationResistance, CurvePoint, DamageChannels, DamageParameters, DegradationCurve,
    HealthLink, VehicleArmor, VehicleArmorDamage,
};

/// Original vehicle armor rows and the scope of their original evidence.
pub type VehicleArmorSnapshot<'source> = EquipmentSnapshot<'source, VehicleArmor<'source>>;

/// Reads original vehicle armor by physical owner without loading localization or world data.
///
/// # Errors
/// Returns input, source-evidence, Archive-change or cancellation errors.
pub fn with_vehicle_armor<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(VehicleArmorSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::VehicleArmorRows, T>(
        archive,
        None,
        selection,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

/// Supplies the same canonical rows from an already decoded Entry of this Archive.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_vehicle_armor_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(VehicleArmorSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<rows::VehicleArmorRows, T>(
        archive,
        Some(entry),
        selection,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        inspect,
    )
}

impl CatalogQuerySession<'_> {
    /// Reads original vehicle armor through this Archive session.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_vehicle_armor<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(VehicleArmorSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_vehicle_armor(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads original vehicle armor from this parsed DataForge.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_vehicle_armor<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(VehicleArmorSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::with_parsed::<rows::VehicleArmorRows, T>(
                archive,
                entry,
                forge,
                selection,
                &[],
                cancel,
                inspect,
            )
        })
    }
}

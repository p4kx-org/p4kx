// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_export::catalog_queries::equipment::vehicle_armor::{
    ArmorDamageInfo, ArmorDamageSource, HealthLink, VehicleArmor, VehicleArmorDamage,
    VehicleArmorSnapshot, with_vehicle_armor_from,
};
use p4kx_export::catalog_queries::{
    CatalogRecordSelection, DatasetAvailability, SourceArenaOutput, SourceField, SourcePayload,
    SourceRecordReference, SourceWeakReference, SourceWeakTarget,
};
use p4kx_extraction::{
    Build, DataForgeOrigin, InputScope, PhysicalSourceKind, PhysicalSourceRef, SourceInventory,
    SourceLocation, SourceUseDescription, SourceUseRef, SourceUseRole, SourceValueState,
    SourcedValue,
};

use super::equipment_input;
use crate::real_archive_catalog_table_parity::TestResult;
use crate::real_archive_vehicle_armor::ARMOR;
use crate::real_archive_verification::print_line;

#[test]
#[ignore = "real Archive: configure the installed Build and measure a fresh process"]
fn installed_vehicle_armor_full_cold_query() -> TestResult {
    equipment_input::with_archive(|archive| {
        let input = equipment_input::dataforge(archive)?;
        let DataForgeOrigin::Archive(origin) = input.origin() else {
            return Err("Vehicle Armor requires the original DataForge Entry".into());
        };
        if !input.origin().matches_archive(archive) || origin.ancestors().len() != 0 {
            return Err("Vehicle Armor requires the root Archive's exact DataForge Entry".into());
        }
        with_vehicle_armor_from(
            archive,
            &input,
            CatalogRecordSelection::All,
            None,
            |snapshot| verify(&snapshot, archive.build(), origin.entry().index()),
        )?
    })
}

fn verify(
    snapshot: &VehicleArmorSnapshot<'_>,
    build: Option<&Build>,
    dataforge_entry: usize,
) -> TestResult {
    let coverage = snapshot.coverage();
    if snapshot.build() != build
        || !matches!(snapshot.availability(), DatasetAvailability::Available)
        || snapshot.rows().is_empty()
        || !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
    {
        return Err(format!(
            "Vehicle Armor SDK coverage: rows={} unreadableHeaders={} unreadableBodies={} incompleteFields={}",
            snapshot.rows().len(),
            coverage.unreadable_headers.len(),
            coverage.unreadable_bodies.len(),
            coverage.incomplete_fields.len(),
        ).into());
    }
    let inventory = snapshot.source_inventory();
    let evidence = Evidence::new(inventory, dataforge_entry)?;
    let mut owners = HashSet::with_capacity(snapshot.rows().len());
    for row in snapshot.rows() {
        if !owners.insert(evidence.row(row)?) {
            return Err("Vehicle Armor duplicated a physical Record and Components slot".into());
        }
    }
    print_line(&format!(
        "Vehicle Armor SDK cold: rows={} sourceUses={} physicalSources={}",
        owners.len(),
        inventory.use_count(),
        inventory.source_count(),
    ));
    Ok(())
}

struct Evidence<'inventory> {
    entry: &'inventory PhysicalSourceRef,
    uses: HashMap<&'inventory SourceUseRef, &'inventory SourceUseDescription>,
}

impl<'inventory> Evidence<'inventory> {
    fn new(inventory: &'inventory SourceInventory, entry: usize) -> TestResult<Self> {
        let mut matches = inventory.iter_sources().filter(|source| {
            source.ancestors().is_empty()
                && match source.kind() {
                    PhysicalSourceKind::ArchiveEntry(details) => details.entry_index() == entry,
                    PhysicalSourceKind::ArchiveEntryProof(proof) => {
                        usize::try_from(proof.entry_index().get()) == Ok(entry)
                    }
                    _ => false,
                }
        });
        let source = matches
            .next()
            .ok_or("Vehicle Armor lacks the original DataForge Entry")?;
        let uses: HashMap<_, _> = inventory
            .iter_uses()
            .map(|usage| (usage.id(), usage))
            .collect();
        if matches.next().is_some() || uses.len() != inventory.use_count() {
            return Err("Vehicle Armor has an ambiguous physical source inventory".into());
        }
        Ok(Self {
            entry: source.id(),
            uses,
        })
    }

    fn cell<'value, T>(&self, cell: &'value SourcedValue<T>) -> TestResult<Option<&'value T>> {
        let origin = cell.origin();
        let ids = origin.source_uses_refs();
        if !origin.source_set_complete()
            || !matches!(
                cell.state(),
                SourceValueState::Present | SourceValueState::Absent | SourceValueState::Null
            )
            || (cell.state() != SourceValueState::Absent && ids.is_empty())
        {
            return Err("Vehicle Armor has an incomplete typed field or source set".into());
        }
        for id in ids {
            let usage = self
                .uses
                .get(id)
                .ok_or("Vehicle Armor has an unbound source use")?;
            if usage.source() != self.entry
                || usage.scope() != InputScope::Payload
                || !matches!(
                    usage.role(),
                    SourceUseRole::Payload | SourceUseRole::Selector | SourceUseRole::Conversion
                )
                || !matches!(
                    usage.location(),
                    SourceLocation::DataForge { .. } | SourceLocation::Subresource { .. }
                )
            {
                return Err(
                    "Vehicle Armor field evidence escaped the original DataForge payload".into(),
                );
            }
        }
        Ok(cell.value())
    }

    fn coordinate<T>(
        &self,
        cell: &SourcedValue<T>,
        role: SourceUseRole,
        namespace: &str,
        field: &str,
        verify: impl Fn(&[u64]) -> bool,
    ) -> TestResult {
        self.cell(cell)?
            .ok_or("Vehicle Armor lacks a physical owner field")?;
        let [id] = cell.origin().source_uses_refs() else {
            return Err("Vehicle Armor needs exactly one physical owner coordinate".into());
        };
        let usage = self
            .uses
            .get(id)
            .ok_or("Vehicle Armor has an unbound owner use")?;
        match usage.location() {
            SourceLocation::Subresource {
                namespace: actual_namespace,
                indices,
                field: actual_field,
            } if usage.role() == role
                && actual_namespace == namespace
                && actual_field == field
                && verify(indices) =>
            {
                Ok(())
            }
            _ => Err("Vehicle Armor lacks the expected physical owner coordinate".into()),
        }
    }

    fn row(&self, row: &VehicleArmor<'_>) -> TestResult<(u32, u32)> {
        let identity = self.owner_identity(row)?;
        if self.cell(row.source_type())? != Some(&ARMOR) {
            return Err("Vehicle Armor row has an unexpected original component type".into());
        }
        self.cell(row.signal_infrared())?;
        self.cell(row.signal_electromagnetic())?;
        self.cell(row.signal_cross_section())?;
        if let Some(damage) = self.cell(row.damage_multiplier())? {
            self.damage(damage)?;
        }
        if let Some(resistance) = self.cell(row.armor_penetration_resistance())? {
            self.cell(resistance.source_type())?;
            self.cell(resistance.base_penetration_reduction())?;
            if let Some(absorption) = self.cell(resistance.penetration_absorption_for_type())? {
                self.damage_info(absorption)?;
            }
            self.arena(resistance.additional_fields())?;
        }
        if let Some(deflection) = self.cell(row.armor_deflection())? {
            self.cell(deflection.source_type())?;
            if let Some(value) = self.cell(deflection.deflection_value())? {
                self.damage_info(value)?;
            }
            if let Some(health) = self.cell(deflection.health_curve())? {
                self.cell(health.source_type())?;
                let curve = health.curve();
                self.cell(&curve.use_lut)?;
                if let Some(points) = self.cell(&curve.points)? {
                    for point in points {
                        if let Some(point) = self.cell(point)? {
                            self.cell(&point.x)?;
                            self.cell(&point.y)?;
                        }
                    }
                }
                self.arena(health.additional_fields())?;
            }
            self.arena(deflection.additional_fields())?;
        }
        self.arena(row.additional_fields())?;
        Ok(identity)
    }

    fn owner_identity(&self, row: &VehicleArmor<'_>) -> TestResult<(u32, u32)> {
        let owner = row.owner();
        let record = *owner
            .record_index()
            .value()
            .ok_or("Vehicle Armor has no physical Record")?;
        let component = *owner
            .component_ordinal()
            .value()
            .ok_or("Vehicle Armor has no physical Components slot")?;
        let index = u64::from(record);
        let header = |indices: &[u64]| indices == [index];
        self.coordinate(
            owner.record_index(),
            SourceUseRole::Selector,
            "dataforgeRecordHeader",
            "recordIndex",
            header,
        )?;
        self.coordinate(
            owner.guid(),
            SourceUseRole::Payload,
            "dataforgeRecordHeader",
            "guid",
            header,
        )?;
        self.coordinate(
            owner.component_ordinal(),
            SourceUseRole::Conversion,
            "dataforgeValue",
            "$value",
            |indices| matches!(indices, [owner, _] if *owner == index),
        )?;
        Ok((record, component))
    }

    fn damage(&self, damage: &VehicleArmorDamage<'_>) -> TestResult {
        match damage {
            VehicleArmorDamage::Base { source } | VehicleArmorDamage::Unknown { source } => {
                self.damage_source(source)
            }
            VehicleArmorDamage::Info { value } => self.damage_info(value),
            VehicleArmorDamage::Parameters { source, value } => {
                self.damage_source(source)?;
                self.cell(value.damage_total())?;
                match self.cell(value.damage_macro())? {
                    Some(HealthLink::RecordReference { value }) => self.record_reference(value)?,
                    Some(HealthLink::WeakReference { value }) => self.weak_reference(value)?,
                    Some(_) => return Err("Vehicle Armor has an unsupported damage link".into()),
                    None => {}
                }
                Ok(())
            }
            _ => Err("Vehicle Armor has an unsupported damage shape".into()),
        }
    }

    fn damage_source(&self, source: &ArmorDamageSource<'_>) -> TestResult {
        self.cell(source.source_type())?;
        self.arena(source.additional_fields())
    }

    fn damage_info(&self, info: &ArmorDamageInfo<'_>) -> TestResult {
        let channels = info.channels();
        for cell in [
            *channels.physical(),
            *channels.energy(),
            *channels.distortion(),
            *channels.thermal(),
            *channels.biochemical(),
            *channels.stun(),
        ] {
            self.cell(cell)?;
        }
        self.damage_source(info.source())
    }

    fn record_reference(&self, reference: &SourceRecordReference<'_>) -> TestResult {
        self.cell(&reference.guid())?;
        self.cell(reference.target_record_index())?;
        self.cell(reference.record_path())?;
        self.cell(reference.target_type())?;
        Ok(())
    }

    fn weak_reference(&self, reference: &SourceWeakReference<'_>) -> TestResult {
        self.cell(reference.target_record_index())?;
        self.cell(reference.record_guid())?;
        self.cell(reference.record_path())?;
        match reference.target() {
            SourceWeakTarget::Record { source_type } => {
                self.cell(source_type)?;
            }
            SourceWeakTarget::Instance {
                source_type,
                location,
            } => {
                self.cell(source_type)?;
                self.cell(location)?;
            }
            _ => return Err("Vehicle Armor has an unsupported retained pointer target".into()),
        }
        Ok(())
    }

    fn arena(&self, arena: &SourceArenaOutput<'_>) -> TestResult {
        self.fields(arena, arena.roots())?;
        for node in arena.values() {
            match self.cell(node.value())? {
                Some(SourcePayload::Struct {
                    source_type,
                    fields,
                }) => {
                    self.cell(source_type)?;
                    self.fields(arena, fields)?;
                }
                Some(SourcePayload::Array { elements }) => {
                    if elements
                        .iter()
                        .any(|element| arena.value(*element).is_none())
                    {
                        return Err("Vehicle Armor has an unbound retained array element".into());
                    }
                }
                Some(SourcePayload::RecordReference { value }) => self.record_reference(value)?,
                Some(SourcePayload::WeakReference { value }) => self.weak_reference(value)?,
                _ => {}
            }
        }
        Ok(())
    }

    fn fields(&self, arena: &SourceArenaOutput<'_>, fields: &[SourceField<'_>]) -> TestResult {
        for field in fields {
            self.cell(field.name())?;
            self.cell(field.declaration().data_type())?;
            self.cell(field.declaration().is_array())?;
            self.cell(field.declaration().target())?;
            if arena.value(field.value()).is_none() {
                return Err("Vehicle Armor has an unbound retained field".into());
            }
        }
        Ok(())
    }
}

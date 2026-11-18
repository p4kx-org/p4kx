// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CanonicalRecordHeader, CatalogRecordReference, QualityCurveSource, QualityOverridePayload,
    QualityOverridesSource, QualityPayload, QualityQuantizationSource, QualitySource,
    ResourceCraftingData, ResourceDefinition, ResourceDensityPayload, ResourceDensitySource,
    ResourceDensityUnit, ResourceDensityUnitPayload, ResourceProperty, ResourcePropertyPayload,
};
use p4kx_extraction::{SourceInventory, SourcedValue};
use serde_json::Value;

#[path = "resource_consumer_wire.rs"]
mod wire;

use wire::{TestResult, Wire};

/// Checks every public Resource getter against its canonical wire row and typed source inventory.
pub(crate) fn visit_resource(
    row: &ResourceDefinition<'_>,
    inventory: &SourceInventory,
    expected: &Value,
) -> TestResult {
    let wire = Wire::new(expected, inventory);
    visit_header(row.source(), wire)?;
    wire.field("name")?.leaf(row.name())?;
    wire.field("description")?.leaf(row.description())?;
    wire.field("groupGuids")?
        .cell(row.group_guids(), |groups, payload| {
            payload.array(groups, |guid, payload| payload.raw(&guid.get().to_string()))
        })?;
    wire.field("refinedVersion")?
        .cell(row.refined_version(), visit_reference)?;
    wire.field("density")?.cell(row.density(), visit_density)?;
    wire.field("defaultThumbnailPath")?
        .leaf(row.default_thumbnail_path())?;
    wire.field("defaultThumbnailPathSVG")?
        .leaf(row.default_thumbnail_path_svg())?;
    wire.field("rttThumbnailEntityClass")?
        .cell(row.rtt_thumbnail_entity_class(), visit_reference)?;
    wire.field("validateDefaultCargoBox")?
        .leaf(row.validate_default_cargo_box())?;
    wire.field("defaultCargoContainers")?.cell(
        row.default_cargo_containers(),
        |slots, payload| {
            payload.array(slots, |slot, payload| {
                payload.field("slot")?.leaf(slot.slot())?;
                payload
                    .field("entityClass")?
                    .cell(slot.entity_class(), visit_reference)
            })
        },
    )?;
    wire.field("properties")?
        .cell(row.properties(), |properties, payload| {
            payload.array(properties, visit_property)
        })
}

/// Checks a typed cell's contribution closure within its exact source inventory.
pub(crate) fn visit_leaf<T>(cell: &SourcedValue<T>, inventory: &SourceInventory) -> TestResult {
    wire::visit_leaf(cell, inventory)
}

fn visit_header(header: &CanonicalRecordHeader<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("guid")?.guid(&header.guid())?;
    wire.field("recordName")?.leaf(header.record_name())?;
    wire.field("className")?.leaf(header.class_name())?;
    wire.field("recordPath")?.leaf(header.record_path())?;
    wire.field("recordIndex")?.leaf(header.record_index())?;
    wire.field("sourceType")?.leaf(header.source_type())?;
    wire.field("fileName")?.leaf(header.file_name())?;
    wire.field("metadataTag")?.leaf(header.metadata_tag())
}

fn visit_reference(reference: &CatalogRecordReference, wire: Wire<'_>) -> TestResult {
    wire.field("status")?.raw(reference.status())?;
    wire.field("guid")?.guid(&reference.guid())?;
    wire.field("targetRecordIndex")?
        .leaf(reference.target_record_index())?;
    wire.field("recordPath")?.leaf(reference.record_path())?;
    wire.field("targetType")?.leaf(reference.target_type())
}

fn visit_density(density: &ResourceDensitySource<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("sourceType")?.leaf(density.source_type())?;
    wire.field("payload")?
        .cell(density.payload(), |payload, wire| match payload {
            ResourceDensityPayload::Density(value) => {
                wire.tag("density")?;
                wire.field("densityUnit")?
                    .cell(value.density_unit(), visit_density_unit)
            }
            ResourceDensityPayload::MolarMass(value) => {
                wire.tag("molarMass")?;
                wire.field("molarMass")?
                    .cell(value.molar_mass(), |value, wire| wire.float32(value))
            }
            ResourceDensityPayload::EmptyBase => wire.tag("emptyBase"),
            _ => Err("a new density branch requires a typed traversal".into()),
        })
}

fn visit_density_unit(unit: &ResourceDensityUnit<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("sourceType")?.leaf(unit.source_type())?;
    wire.field("payload")?
        .cell(unit.payload(), |payload, wire| match payload {
            ResourceDensityUnitPayload::GramsPerCubicCentimeter(value) => {
                wire.tag("gramsPerCubicCentimeter")?;
                wire.field("value")?
                    .cell(value.value(), |value, wire| wire.float32(value))
            }
            ResourceDensityUnitPayload::KilogramsPerCubicMeter(value) => {
                wire.tag("kilogramsPerCubicMeter")?;
                wire.field("value")?
                    .cell(value.value(), |value, wire| wire.float32(value))
            }
            ResourceDensityUnitPayload::EmptyBase => wire.tag("emptyBase"),
            _ => Err("a new density-unit branch requires a typed traversal".into()),
        })
}

fn visit_property(property: &ResourceProperty<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("sourceType")?.leaf(property.source_type())?;
    wire.field("name")?.leaf(property.name())?;
    wire.field("payload")?
        .cell(property.payload(), |payload, wire| match payload {
            ResourcePropertyPayload::Base => wire.tag("base"),
            ResourcePropertyPayload::Volatility(value) => {
                wire.tag("volatility")?;
                wire.field("volatility")?
                    .cell(value.volatility(), |value, wire| wire.float32(value))?;
                wire.field("healthDecayPerSecond")?
                    .cell(value.health_decay_per_second(), |value, wire| {
                        wire.float32(value)
                    })
            }
            ResourcePropertyPayload::CraftingData(value) => {
                wire.tag("craftingData")?;
                visit_crafting(value, wire)
            }
            _ => Err("a new resource-property branch requires a typed traversal".into()),
        })
}

fn visit_crafting(data: &ResourceCraftingData<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("qualityDistribution")?
        .cell(data.quality_distribution(), |source, wire| {
            visit_quality(source, wire, visit_curve)
        })?;
    wire.field("qualityLocationOverride")?
        .cell(data.quality_location_override(), |source, wire| {
            visit_quality(source, wire, visit_overrides)
        })?;
    wire.field("qualityQuantization")?
        .cell(data.quality_quantization(), |source, wire| {
            visit_quality(source, wire, visit_quantization)
        })
}

fn visit_quality<'expected, T>(
    source: &QualitySource<'_, T>,
    wire: Wire<'expected>,
    visit: impl FnOnce(&T, Wire<'expected>) -> TestResult,
) -> TestResult {
    wire.field("sourceType")?.leaf(source.source_type())?;
    wire.field("payload")?
        .cell(source.payload(), |payload, wire| match payload {
            QualityPayload::Inline(value) => {
                wire.tag("inline")?;
                wire.field("value")?.cell(value.value(), visit)
            }
            QualityPayload::RecordRef(value) => {
                wire.tag("recordRef")?;
                wire.field("reference")?
                    .cell(value.reference(), visit_reference)
            }
            QualityPayload::EmptyBase => wire.tag("emptyBase"),
            _ => Err("a new quality-ownership branch requires a typed traversal".into()),
        })
}

fn visit_curve(curve: &QualityCurveSource, wire: Wire<'_>) -> TestResult {
    wire.field("min")?
        .cell(curve.min(), |number, wire| wire.number(number))?;
    wire.field("max")?
        .cell(curve.max(), |number, wire| wire.number(number))?;
    wire.field("mean")?
        .cell(curve.mean(), |number, wire| wire.number(number))?;
    wire.field("stddev")?
        .cell(curve.stddev(), |number, wire| wire.number(number))
}

fn visit_overrides(overrides: &QualityOverridesSource<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("locationOverrideList")?
        .cell(overrides.location_override_list(), |entries, wire| {
            wire.array(entries, |source, wire| {
                wire.field("sourceType")?.leaf(source.source_type())?;
                wire.field("payload")?
                    .cell(source.payload(), |payload, wire| match payload {
                        QualityOverridePayload::Entry(fields) => {
                            wire.tag("entry")?;
                            wire.field("location")?
                                .cell(fields.location(), visit_reference)?;
                            wire.field("qualityDistribution")?
                                .cell(fields.quality_distribution(), |source, wire| {
                                    visit_quality(source, wire, visit_curve)
                                })
                        }
                        QualityOverridePayload::EmptyBase => wire.tag("emptyBase"),
                        _ => Err("a new quality-override branch requires a typed traversal".into()),
                    })
            })
        })
}

fn visit_quantization(quantization: &QualityQuantizationSource<'_>, wire: Wire<'_>) -> TestResult {
    wire.field("bands")?
        .cell(quantization.bands(), |bands, wire| {
            wire.array(bands, |band, wire| {
                wire.field("sourceType")?.leaf(band.source_type())?;
                wire.field("start")?
                    .cell(band.start(), |number, wire| wire.number(number))?;
                wire.field("end")?
                    .cell(band.end(), |number, wire| wire.number(number))?;
                wire.field("mappedValue")?
                    .cell(band.mapped_value(), |number, wire| wire.number(number))
            })
        })
}

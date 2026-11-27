// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};

/// The declared target of an owned location-override entry.
#[derive(Clone, Copy)]
pub(crate) enum OverrideEntryTarget {
    /// The pointer declares the concrete entry type.
    Concrete,
    /// The pointer declares the entry base type.
    Base,
}

/// Adds the synthetic resource declarations; the caller supplies external Reference targets.
pub(crate) fn declarations(
    builder: DataForgeBuilder,
    target: OverrideEntryTarget,
) -> DataForgeBuilder {
    let entry_type = match target {
        OverrideEntryTarget::Concrete => "CraftingQualityLocationOverrideEntry",
        OverrideEntryTarget::Base => "CraftingQualityLocationOverrideEntry_Base",
    };
    let builder = crafting_quality_distribution_normal(builder);
    let builder = crafting_quality_distribution_uniform(builder);
    let builder = crafting_quality_distribution_base(builder);
    let builder = crafting_quality_distribution_base_non_ref(builder);
    let builder = crafting_quality_distribution_record_ref(builder);
    let builder = crafting_quality_location_override(builder, entry_type);
    let builder = crafting_quality_location_override_entry(builder);
    let builder = crafting_quality_location_override_entry_base(builder);
    let builder = crafting_quality_location_override_entry_base_non_ref(builder);
    let builder = crafting_quality_location_override_base(builder);
    let builder = crafting_quality_location_override_base_non_ref(builder);
    let builder = crafting_quality_location_override_record_ref(builder);
    let builder = crafting_quality_quantization(builder);
    let builder = crafting_quality_quantization_band(builder);
    let builder = crafting_quality_quantization_base(builder);
    let builder = crafting_quality_quantization_base_non_ref(builder);
    crafting_quality_quantization_record_ref(builder)
}

fn crafting_quality_distribution_normal(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("CraftingQualityDistributionNormal")
            .parent("CraftingQualityDistribution_Base_NonRef")
            .field("min", TestType::Int32)
            .field("max", TestType::Int32)
            .field("mean", TestType::Single)
            .field("stddev", TestType::Single),
    )
}

fn crafting_quality_distribution_uniform(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("CraftingQualityDistributionUniform")
            .parent("CraftingQualityDistribution_Base_NonRef")
            .field("min", TestType::Int32)
            .field("max", TestType::Int32),
    )
}

fn crafting_quality_distribution_base(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(TestStruct::new("CraftingQualityDistribution_Base"))
}

fn crafting_quality_distribution_base_non_ref(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("CraftingQualityDistribution_Base_NonRef")
            .parent("CraftingQualityDistribution_Base"),
    )
}

fn crafting_quality_distribution_record_ref(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("CraftingQualityDistribution_RecordRef")
            .parent("CraftingQualityDistribution_Base")
            .field(
                "qualityDistributionRecord",
                TestType::ReferenceTo("CraftingQualityDistributionRecord".to_owned()),
            ),
    )
}

fn crafting_quality_location_override(
    builder: DataForgeBuilder,
    entry_type: &str,
) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("CraftingQualityLocationOverride")
            .parent("CraftingQualityLocationOverride_Base_NonRef")
            .array(
                "locationOverrideList",
                TestType::StrongPointer(entry_type.to_owned()),
            ),
    )
}

fn crafting_quality_location_override_entry(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("CraftingQualityLocationOverrideEntry")
            .parent("CraftingQualityLocationOverrideEntry_Base_NonRef")
            .field(
                "location",
                TestType::ReferenceTo("StarMapObject".to_owned()),
            )
            .field(
                "qualityDistribution",
                TestType::StrongPointer("CraftingQualityDistribution_Base".to_owned()),
            ),
    )
}

fn crafting_quality_location_override_entry_base(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(TestStruct::new("CraftingQualityLocationOverrideEntry_Base"))
}

fn crafting_quality_location_override_entry_base_non_ref(
    builder: DataForgeBuilder,
) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("CraftingQualityLocationOverrideEntry_Base_NonRef")
            .parent("CraftingQualityLocationOverrideEntry_Base"),
    )
}

fn crafting_quality_location_override_base(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(TestStruct::new("CraftingQualityLocationOverride_Base"))
}

fn crafting_quality_location_override_base_non_ref(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("CraftingQualityLocationOverride_Base_NonRef")
            .parent("CraftingQualityLocationOverride_Base"),
    )
}

fn crafting_quality_location_override_record_ref(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("CraftingQualityLocationOverride_RecordRef")
            .parent("CraftingQualityLocationOverride_Base")
            .field(
                "locationOverrideRecord",
                TestType::ReferenceTo("CraftingQualityLocationOverrideRecord".to_owned()),
            ),
    )
}

fn crafting_quality_quantization(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("CraftingQualityQuantization")
            .parent("CraftingQualityQuantization_Base_NonRef")
            .array(
                "bands",
                TestType::Class("CraftingQualityQuantizationBand".to_owned()),
            ),
    )
}

fn crafting_quality_quantization_band(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("CraftingQualityQuantizationBand")
            .field("start", TestType::Int32)
            .field("end", TestType::Int32)
            .field("mappedValue", TestType::Int32),
    )
}

fn crafting_quality_quantization_base(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(TestStruct::new("CraftingQualityQuantization_Base"))
}

fn crafting_quality_quantization_base_non_ref(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("CraftingQualityQuantization_Base_NonRef")
            .parent("CraftingQualityQuantization_Base"),
    )
}

fn crafting_quality_quantization_record_ref(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("CraftingQualityQuantization_RecordRef")
            .parent("CraftingQualityQuantization_Base")
            .field(
                "qualityQuantizationRecord",
                TestType::ReferenceTo("CraftingQualityQuantizationRecord".to_owned()),
            ),
    )
}

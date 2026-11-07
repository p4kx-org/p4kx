// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};

use crate::resource_density_schema_fixture;
use crate::resource_quality_schema_fixture::{self, OverrideEntryTarget};

/// Adds the synthetic resource declarations; the caller supplies external Reference targets.
pub(crate) fn declarations(
    builder: DataForgeBuilder,
    target: OverrideEntryTarget,
) -> DataForgeBuilder {
    let builder = resource_density_schema_fixture::declarations(builder);
    let builder = resource_quality_schema_fixture::declarations(builder, target);
    let builder = resource_type(builder);
    let builder = resource_type_crafting_data(builder);
    let builder = resource_type_group(builder);
    let builder = resource_type_properties(builder);
    let builder = resource_type_volatility(builder);
    sresource_type_default_cargo_containers(builder)
}

fn resource_type(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("ResourceType")
            .field("displayName", TestType::Locale)
            .field("description", TestType::Locale)
            .field(
                "densityType",
                TestType::StrongPointer("ResourceTypeDensityType".to_owned()),
            )
            .field("defaultThumbnailPath", TestType::String)
            .field("defaultThumbnailPathSVG", TestType::String)
            .field(
                "rttThumbnailEntityClass",
                TestType::ReferenceTo("EntityClassDefinition".to_owned()),
            )
            .array(
                "properties",
                TestType::StrongPointer("ResourceTypeProperties".to_owned()),
            )
            .field(
                "refinedVersion",
                TestType::ReferenceTo("ResourceType".to_owned()),
            )
            .field("validateDefaultCargoBox", TestType::Bool)
            .field(
                "defaultCargoContainers",
                TestType::StrongPointer("SResourceTypeDefaultCargoContainers".to_owned()),
            ),
    )
}

fn resource_type_crafting_data(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("ResourceTypeCraftingData")
            .parent("ResourceTypeProperties")
            .field(
                "qualityDistribution",
                TestType::StrongPointer("CraftingQualityDistribution_Base".to_owned()),
            )
            .field(
                "qualityLocationOverride",
                TestType::StrongPointer("CraftingQualityLocationOverride_Base".to_owned()),
            )
            .field(
                "qualityQuantization",
                TestType::StrongPointer("CraftingQualityQuantization_Base".to_owned()),
            ),
    )
}

fn resource_type_group(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("ResourceTypeGroup")
            .field("displayName", TestType::Locale)
            .field("description", TestType::Locale)
            .field("defaultThumbnailPath", TestType::String)
            .field("defaultThumbnailPathSVG", TestType::String)
            .array(
                "groups",
                TestType::ReferenceTo("ResourceTypeGroup".to_owned()),
            )
            .array(
                "resources",
                TestType::ReferenceTo("ResourceType".to_owned()),
            ),
    )
}

fn resource_type_properties(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(TestStruct::new("ResourceTypeProperties").field("name", TestType::String))
}

fn resource_type_volatility(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("ResourceTypeVolatility")
            .parent("ResourceTypeProperties")
            .field("volatility", TestType::Single)
            .field("healthDecayPerSecond", TestType::Single),
    )
}

fn sresource_type_default_cargo_containers(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("SResourceTypeDefaultCargoContainers")
            .field(
                "one_eighthSCU",
                TestType::ReferenceTo("EntityClassDefinition".to_owned()),
            )
            .field(
                "one_quarterSCU",
                TestType::ReferenceTo("EntityClassDefinition".to_owned()),
            )
            .field(
                "one_halfSCU",
                TestType::ReferenceTo("EntityClassDefinition".to_owned()),
            )
            .field(
                "oneSCU",
                TestType::ReferenceTo("EntityClassDefinition".to_owned()),
            )
            .field(
                "twoSCU",
                TestType::ReferenceTo("EntityClassDefinition".to_owned()),
            )
            .field(
                "fourSCU",
                TestType::ReferenceTo("EntityClassDefinition".to_owned()),
            )
            .field(
                "eightSCU",
                TestType::ReferenceTo("EntityClassDefinition".to_owned()),
            )
            .field(
                "sixteenSCU",
                TestType::ReferenceTo("EntityClassDefinition".to_owned()),
            )
            .field(
                "twentyFourSCU",
                TestType::ReferenceTo("EntityClassDefinition".to_owned()),
            )
            .field(
                "thirtyTwoSCU",
                TestType::ReferenceTo("EntityClassDefinition".to_owned()),
            ),
    )
}

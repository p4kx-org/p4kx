// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};

/// Adds the synthetic resource declarations; the caller supplies external Reference targets.
pub(crate) fn declarations(builder: DataForgeBuilder) -> DataForgeBuilder {
    let builder = base_density_unit(builder);
    let builder = grams_per_cubic_centimeter(builder);
    let builder = kilograms_per_cubic_meter(builder);
    let builder = resource_type_density(builder);
    let builder = resource_type_density_type(builder);
    resource_type_molar_mass(builder)
}

fn base_density_unit(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(TestStruct::new("BaseDensityUnit"))
}

fn grams_per_cubic_centimeter(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("GramsPerCubicCentimeter")
            .parent("BaseDensityUnit")
            .field("gramsPerCubicCentimeter", TestType::Single),
    )
}

fn kilograms_per_cubic_meter(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("KilogramsPerCubicMeter")
            .parent("BaseDensityUnit")
            .field("kilogramsPerCubicMeter", TestType::Single),
    )
}

fn resource_type_density(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("ResourceTypeDensity")
            .parent("ResourceTypeDensityType")
            .field(
                "densityUnit",
                TestType::StrongPointer("BaseDensityUnit".to_owned()),
            ),
    )
}

fn resource_type_density_type(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(TestStruct::new("ResourceTypeDensityType"))
}

fn resource_type_molar_mass(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder.structure(
        TestStruct::new("ResourceTypeMolarMass")
            .parent("ResourceTypeDensityType")
            .field("molarMass", TestType::Single),
    )
}

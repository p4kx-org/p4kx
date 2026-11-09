// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// Numeric storage retained by shared canonical resource and quality field contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourceNumberContract {
    Single,
    Number,
    Int32,
}

/// Returns the exact owner-and-field contract used by both projection and inspection.
pub(crate) fn field_contract(owner: &str, field: &str) -> Option<ResourceNumberContract> {
    match (owner, field) {
        ("GramsPerCubicCentimeter", "gramsPerCubicCentimeter")
        | ("KilogramsPerCubicMeter", "kilogramsPerCubicMeter")
        | ("ResourceTypeMolarMass", "molarMass")
        | ("ResourceTypeVolatility", "volatility" | "healthDecayPerSecond") => {
            Some(ResourceNumberContract::Single)
        }
        (
            "CraftingQualityDistributionNormal" | "CraftingQualityDistributionUniform",
            "mean" | "stddev",
        ) => Some(ResourceNumberContract::Number),
        (
            "CraftingQualityDistributionNormal" | "CraftingQualityDistributionUniform",
            "min" | "max",
        )
        | ("CraftingQualityQuantizationBand", "start" | "end" | "mappedValue") => {
            Some(ResourceNumberContract::Int32)
        }
        _ => None,
    }
}

impl ResourceNumberContract {
    pub(crate) const fn captures(self, value: &p4kx_extraction::Value<'_>) -> bool {
        use p4kx_extraction::Value;

        match self {
            Self::Single => matches!(value, Value::Single(_)),
            Self::Number | Self::Int32 => matches!(value, Value::Single(_) | Value::Double(_)),
        }
    }
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::Value;

    use super::field_contract;

    #[test]
    fn only_the_exact_scalar_storage_contract_is_exempt_from_finite_projection() {
        let single = field_contract("ResourceTypeMolarMass", "molarMass");
        assert!(
            single.is_some_and(|contract| contract.captures(&Value::Single(f32::NAN))),
            "Single storage is captured by the Single contract"
        );
        assert!(
            !single.is_some_and(|contract| contract.captures(&Value::Double(f64::NAN))),
            "Single contracts do not exempt Double storage"
        );
        assert!(
            !single.is_some_and(|contract| contract.captures(&Value::Array(Box::new([])))),
            "scalar contracts do not exempt arrays"
        );
        assert!(
            field_contract("ResourceTypeMolarMass", "extra.molarMass").is_none(),
            "field names are exact source names"
        );
        assert!(
            field_contract("OtherMolarMass", "molarMass").is_none(),
            "owner types are exact source types"
        );
        let number = field_contract("CraftingQualityDistributionNormal", "mean");
        assert!(
            number.is_some_and(|contract| contract.captures(&Value::Double(f64::NAN))),
            "Number contracts preserve Double storage"
        );
    }
}

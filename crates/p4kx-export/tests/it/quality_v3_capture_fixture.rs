// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestValue};

use crate::quality_v3_fixture_schema as schema;
use crate::quality_v3_fixture_values as values;

pub(crate) const NAN_BITS: u64 = 0x7ff8_0000_0000_2345;
pub(crate) const NAN_HEX: &str = "7ff8000000002345";
pub(crate) const RESOURCE_MEAN: &str = "properties[0].qualityDistribution.mean";

pub(crate) fn curve() -> TestValue {
    values::normal(
        TestValue::Int32(0),
        TestValue::Double(f64::from_bits(NAN_BITS)),
        TestValue::Double(1.25),
    )
}

pub(crate) fn filtered_dependency(alias: bool) -> DataForgeBuilder {
    let source = schema::schema()
        .record(values::quantization(
            values::QUANTIZATION,
            "Quantizer",
            "included/quantizer",
            values::bands(vec![values::band(0, 99, 20)]),
        ))
        .record(values::resource(
            values::RESOURCE_A,
            "excluded/original",
            vec![
                values::distribution_property(curve()),
                values::property(Some(values::QUANTIZATION)),
            ],
        ));
    if alias {
        source.record(values::resource(
            values::RESOURCE_A,
            "included/alias",
            vec![
                values::distribution_property(curve()),
                values::property(None),
            ],
        ))
    } else {
        source
    }
}

pub(crate) fn uncaptured_owner() -> DataForgeBuilder {
    schema::uncaptured_curve_schema().record(
        values::distribution(values::DISTRIBUTION, "uncaptured", curve())
            .value("spareCurve", curve()),
    )
}

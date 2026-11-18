// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestType, TestValue};

use crate::commodity_v3_fixture::TestResult;

use crate::quality_v3_fixture_schema as schema;
use crate::quality_v3_fixture_values as values;

pub(super) const DISTRIBUTION: u128 = 0x1166_0000_0000_4000_8000_0000_0000_c001;
pub(super) const OVERRIDE: u128 = 0x1166_0000_0000_4000_8000_0000_0000_c002;
pub(super) const QUANTIZATION: u128 = 0x1166_0000_0000_4000_8000_0000_0000_c003;
pub(super) const MISSING: u128 = 0x1166_0000_0000_4000_8000_0000_0000_c004;
pub(super) const NULL_DISTRIBUTION: u128 = 0x1166_0000_0000_4000_8000_0000_0000_c005;
pub(super) const DANGLING_DISTRIBUTION: u128 = 0x1166_0000_0000_4000_8000_0000_0000_c006;
pub(super) const NULL_QUANTIZATION: u128 = 0x1166_0000_0000_4000_8000_0000_0000_c007;
pub(super) const NAN_BITS: u64 = 0xfff8_0000_0000_1166;
pub(super) const WIDE_MIN: i64 = 9_007_199_254_740_993;

pub(super) fn database() -> DataForgeBuilder {
    schema::numeric_schema(TestType::Int32, TestType::Double, TestType::Single)
        .record(primary_distribution())
        .record(alias_distribution())
        .record(values::override_record(
            OVERRIDE,
            "original_overrides",
            values::overrides(vec![
                values::override_entry(17),
                TestValue::Null,
                values::override_entry(17),
            ]),
        ))
        .record(values::quantization(
            QUANTIZATION,
            "Primary",
            "primary",
            values::bands(vec![
                values::band(0, 399, 100),
                values::band(400, 1000, 900),
            ]),
        ))
        .record(values::quantization(
            QUANTIZATION,
            "Alias",
            "alias",
            values::reference(
                "CraftingQualityQuantization_RecordRef",
                "qualityQuantizationRecord",
                MISSING,
            ),
        ))
        .record(values::distribution(
            NULL_DISTRIBUTION,
            "null_distribution",
            TestValue::Null,
        ))
        .record(values::distribution(
            DANGLING_DISTRIBUTION,
            "dangling_distribution",
            values::reference(
                "CraftingQualityDistribution_RecordRef",
                "qualityDistributionRecord",
                MISSING,
            ),
        ))
        .record(values::quantization(
            NULL_QUANTIZATION,
            "Null",
            "null_quantization",
            TestValue::Null,
        ))
}

fn alias_distribution() -> TestRecord {
    values::distribution(
        DISTRIBUTION,
        "alias",
        values::normal(
            TestValue::Int32(17),
            TestValue::Double(1.0e300),
            TestValue::Single(0.5),
        ),
    )
}

fn primary_distribution() -> TestRecord {
    values::distribution(
        DISTRIBUTION,
        "primary",
        values::normal(
            TestValue::Int32(i32::MIN),
            TestValue::Double(f64::from_bits(NAN_BITS)),
            TestValue::Single(-0.0),
        ),
    )
}

pub(super) fn wide_distribution() -> DataForgeBuilder {
    schema::numeric_schema(TestType::Int64, TestType::Double, TestType::Double).record(
        values::distribution(
            DISTRIBUTION,
            "wide_original",
            values::normal(
                TestValue::Int64(WIDE_MIN),
                TestValue::Double(1.0e300),
                TestValue::Double(-0.0),
            ),
        ),
    )
}

pub(super) fn open_without_key() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("source.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database().build()))
        .entry(
            TestEntry::stored(Language::English.global_ini_path(), b"unused=Locale\n").encrypted(),
        )
        .entry(
            TestEntry::stored(
                "Data/ObjectContainers/PU/system/synthetic/syntheticsystem.socpak",
                b"unopened-world",
            )
            .encrypted(),
        )
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}

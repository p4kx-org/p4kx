// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value;

use super::{check_unique_guids, definition_identity};
use crate::catalog_v3_consumer as reader;
use crate::commodity_v3_fixture::{self as fixture, TestResult};

const LOCALITY: &str = "MissionLocality";
const DISTINCT_LOCALITY: u128 = 0x1125_0101;

#[test]
fn locality_identity_keeps_physical_alternatives_and_rejects_cross_family_collisions() -> TestResult
{
    let (_, mut catalog) = fixture::build(&database(DISTINCT_LOCALITY))?;

    let [first, second] = reader::rows(&catalog, Kind::MissionLocalities)?.as_slice() else {
        return Err("both physical locality owners".into());
    };
    for row in [first, second] {
        assert_eq!(
            reader::identity(row, Kind::MissionLocalities)?,
            fixture::guid(DISTINCT_LOCALITY),
            "both original locality owners retain their common GUID"
        );
    }
    assert_eq!(
        reader::field(first, Kind::MissionLocalities, "recordPath")?,
        reader::field(second, Kind::MissionLocalities, "recordPath")?,
        "both physical occurrences share the original path"
    );
    check_unique_guids(&catalog)?;
    let localities = catalog
        .pointer_mut("/definitions/missionLocalities/rows")
        .and_then(Value::as_array_mut)
        .ok_or("canonical locality rows")?;
    localities.swap(0, 1);
    assert!(
        definition_identity::check(&catalog, Kind::MissionLocalities).is_err(),
        "swapping the two physical positions of the same GUID must fail"
    );

    let (_, collision) = fixture::build(&database(fixture::QUANTIZATION))?;
    definition_identity::check(&collision, Kind::MissionLocalities)?;
    let error = check_unique_guids(&collision)
        .err()
        .ok_or("the original cross-family GUID collision must fail")?;
    assert_eq!(
        error.to_string(),
        "a missionLocalities GUID also occurs in qualityQuantizations",
        "the cross-family guard rejects the specific original collision"
    );
    Ok(())
}

fn database(guid: u128) -> DataForgeBuilder {
    fixture::database()
        .structure(TestStruct::new(LOCALITY).array("availableLocations", TestType::Reference))
        .record(locality(guid, "First"))
        .record(locality(guid, "Second"))
}

fn locality(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        LOCALITY,
        format!("{LOCALITY}.{name}"),
        "libs/foundry/records/missiondata/pu_missionlocality/fixture.xml",
    )
    .value("availableLocations", TestValue::Array(Vec::new()))
}

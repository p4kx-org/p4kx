// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Mission localization checks against a produced catalog and its original language data.

use p4kx_export::Kind;
use p4kx_testkit::{TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value;

use crate::commodity_v3_fixture as source;

use super::{Fixture, TestResult};

#[test]
fn mission_locale_coverage_checks_produced_broker_texts() -> TestResult {
    let database = source::database()
        .structure(
            TestStruct::new("MissionBrokerEntry")
                .field("title", TestType::Locale)
                .field("description", TestType::Locale),
        )
        .record(
            TestRecord::new(
                0x9e03,
                "MissionBrokerEntry",
                "MissionBrokerEntry.Localized",
                "libs/foundry/records/missionbroker/Localized.xml",
            )
            .value("title", TestValue::text("@resource_name"))
            .value("description", TestValue::text("@resource_description")),
        );
    let mut fixture = Fixture::from_database(&database)?;
    let kind = Kind::MissionBrokerEntries;
    let (english, german) = fixture.coverage(&fixture.catalog, kind)?;
    assert_eq!(
        [english.resolved, german.resolved],
        [2, 2],
        "both authored Broker keys match the language entries in each language"
    );
    assert_eq!(
        [english.total(), german.total()],
        [2, 2],
        "the reader counts each Broker lookup once"
    );
    let lookups = fixture
        .catalog
        .pointer_mut("/configurations/missionBrokerEntries/rows/0/localizations")
        .and_then(Value::as_array_mut)
        .ok_or("actual Broker localization lookups")?;
    let [first, second] = lookups.as_mut_slice() else {
        return Err("expected two produced localizations on the same Broker".into());
    };
    super::replace_value(
        first,
        "/text/value",
        source::at(second, "/text/value")?.clone(),
    )?;
    let Err(error) = fixture.coverage(&fixture.catalog, kind) else {
        return Err("the reader accepted another text target from the same Broker".into());
    };
    assert_eq!(
        error.to_string(),
        "mission localization disagrees with original key or text",
        "an existing GameText target must match the original key"
    );
    Ok(())
}

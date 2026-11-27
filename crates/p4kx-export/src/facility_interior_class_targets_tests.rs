// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::{ClassMembership, ClassSurvey, ClassTarget, ClassTargets, survey};
use crate::CatalogError;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ALIASED: u128 = 0x1063_4000;
const UNIQUE: u128 = 0x1063_4001;
const UNCONFIGURED: u128 = 0x1063_4002;

fn record(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "ScopeFixture",
        format!("ScopeFixture.{name}"),
        format!("fixture/{name}.xml"),
    )
    .value("ok", TestValue::Bool(true))
}

#[test]
fn configured_guid_lookup_respects_unconfigured_physical_aliases() -> TestResult {
    let data = DataForgeBuilder::new()
        .structure(TestStruct::new("ScopeFixture").field("ok", TestType::Bool))
        .record(record(ALIASED, "unconfigured_alias"))
        .record(record(ALIASED, "configured_owner"))
        .record(record(UNIQUE, "configured_unique"))
        .record(record(UNCONFIGURED, "unconfigured_unique"))
        .build();
    let forge = DataForge::open(&data)?;
    let guid = |position| {
        forge
            .record(position)
            .map(|record| record.guid())
            .ok_or("original class GUID")
    };
    let mut targets = ClassTargets::new(&forge, [1, 2], None)?;
    assert_eq!(
        targets.resolve(guid(0)?)?,
        Some(ClassTarget::Ambiguous),
        "an original alias makes an otherwise configured class ambiguous"
    );
    assert_eq!(
        targets.resolve(guid(2)?)?,
        Some(ClassTarget::One(2)),
        "the unique configured physical owner is preserved"
    );
    assert_eq!(
        targets.resolve(guid(3)?)?,
        None,
        "a known unconfigured class does not join"
    );
    assert_eq!(
        targets.resolve(guid(0)?)?,
        Some(ClassTarget::Ambiguous),
        "repeated Entities use the same physical classification"
    );
    Ok(())
}

#[test]
fn partial_membership_stays_distinct_from_complete_absence_and_known_conflict() -> TestResult {
    let unavailable = |_| -> Result<ClassMembership, CatalogError> {
        Ok(ClassMembership {
            configured: false,
            complete: false,
        })
    };
    assert_eq!(
        survey([7], unavailable)?,
        ClassSurvey {
            target: None,
            complete: false
        },
        "unreadable candidate membership cannot prove absence",
    );
    assert_eq!(
        survey([], unavailable)?,
        ClassSurvey {
            target: None,
            complete: true
        },
        "a GUID with no physical alias proves class absence without hydration",
    );
    assert_eq!(
        survey([7, 9], |record| Ok(ClassMembership {
            configured: record == 9,
            complete: record == 9
        }))?,
        ClassSurvey {
            target: Some(ClassTarget::Ambiguous),
            complete: false
        },
        "a known configured owner with a physical alias proves ambiguity without certifying the failed alias",
    );
    Ok(())
}

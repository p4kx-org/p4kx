// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::{DataForge, DataForgeError, Guid};
use p4kx_testkit::{TestEntity, TestValue};

use super::{DEFINITION, DERIVED_OWNER, DERIVED_TARGET, OWNER, TURRET};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

const EXTRA_OWNER: u128 = 0x7316;
const COMPONENTS: &str = "Components";
const OTHER: &str = "FixtureWrongLink";

#[test]
fn unreadable_turret_type_counts_only_reached_discovery_failures() -> TestResult {
    let bytes = corrupt_turret_type()?;
    let mut failed = [physical(&bytes, OWNER)?, physical(&bytes, DERIVED_OWNER)?];
    failed.sort_unstable();
    let derived = physical(&bytes, DERIVED_TARGET)?;
    let base = physical(&bytes, DEFINITION)?;
    let (_folder, archive) = common::open_bytes(bytes, DcbOrigin::Encrypted)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let sdk = session.parsed(None)?;
    sdk.with_turret_health_modifiers(
        CatalogRecordSelection::RecordIndex(derived),
        None,
        |snapshot| {
            assert!(
                snapshot.rows().is_empty(),
                "no compatible component link can be admitted"
            );
            assert!(
                snapshot.coverage().unreadable_headers.is_empty(),
                "the selected definition still has a readable header"
            );
            assert_eq!(
                snapshot.definition_discovery().failed_records(),
                &failed,
                "only the two actual component owners are unclassifiable candidates"
            );
            assert!(
                !snapshot.definition_discovery().is_complete(),
                "a reached unclassifiable declaration prevents complete discovery"
            );
        },
    )?;
    sdk.with_turret_health_modifiers(
        CatalogRecordSelection::RecordIndex(base),
        None,
        |snapshot| {
            let [row] = snapshot.rows() else {
                return Err("the directly selected base modifier survives".into());
            };
            assert_eq!(
                row.source().record_index().value(),
                Some(&base),
                "the isolated base query keeps its physical Record"
            );
            assert!(
                snapshot.definition_discovery().is_complete(),
                "direct base admission does not consult component pointers"
            );
            Ok(())
        },
    )?
}

#[test]
fn derived_discovery_checks_bad_exact_slots_after_valid_components() -> TestResult {
    for family in [TURRET, OTHER] {
        let bytes = with_broken_sibling(family)?;
        let broken = physical(&bytes, EXTRA_OWNER)?;
        let derived = physical(&bytes, DERIVED_TARGET)?;
        let (_folder, archive) = common::open_bytes(bytes, DcbOrigin::Encrypted)?;
        let session =
            CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
        let sdk = session.parsed(None)?;
        sdk.with_turret_health_modifiers(
            CatalogRecordSelection::RecordIndex(derived),
            None,
            |snapshot| {
                assert_eq!(
                    snapshot.rows().len(),
                    1,
                    "the separate healthy owner proves the compatible definition type"
                );
                let expected = if family == TURRET {
                    std::slice::from_ref(&broken)
                } else {
                    &[]
                };
                let discovery = snapshot.definition_discovery();
                assert_eq!(
                    discovery.failed_records(), expected,
                    "{family} should only affect discovery when the broken slot could belong to the selected family"
                );
                assert_eq!(
                    discovery.is_complete(), expected.is_empty(),
                    "{family} uses the actual scoped discovery result"
                );
            },
        )?;
    }
    Ok(())
}

fn corrupt_turret_type() -> TestResult<Vec<u8>> {
    let mut bytes = super::derived_database().build();
    let (index, offset) = {
        let forge = DataForge::open(&bytes)?;
        let declaration = forge
            .structs()
            .find(|item| item.name() == Ok(TURRET))
            .ok_or("the original turret declaration")?;
        let offset = declaration
            .name()?
            .as_ptr()
            .addr()
            .checked_sub(bytes.as_ptr().addr())
            .ok_or("the original turret type name is inside the fixture")?;
        (declaration.index(), offset)
    };
    let first = bytes.get_mut(offset).ok_or("the original type-name byte")?;
    assert_eq!(*first, b'S', "the edit addresses the turret type name");
    *first = 0xff;

    let forge = DataForge::open(&bytes)?;
    assert!(
        forge.pointer_probe(COMPONENTS, TURRET).is_none(),
        "the corrupt declaration has no legacy named probe"
    );
    let corrupt = forge
        .struct_at(index)
        .ok_or("the physical corrupt declaration remains")?;
    assert!(
        corrupt.instance_count() > 0
            && matches!(corrupt.name(), Err(DataForgeError::InvalidString { .. })),
        "a populated component declaration survives with an unreadable name"
    );
    Ok(bytes)
}

fn with_broken_sibling(family: &str) -> TestResult<Vec<u8>> {
    let database = super::derived_database();
    let source = database.build();
    let forge = DataForge::open(&source)?;
    let declaration = forge
        .structs()
        .find(|item| item.name() == Ok(family))
        .ok_or("the actual component declaration")?;
    let bad = TestValue::Raw(
        [declaration.index(), u32::MAX]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect(),
    );
    Ok(database
        .record(
            TestEntity::new(
                EXTRA_OWNER,
                "FixtureSiblingTurret",
                "libs/foundry/records/entities/fixtures/turret_sibling.xml",
            )
            .component(super::turret(TestValue::Reference(DERIVED_TARGET)))
            .component(bad)
            .record(),
        )
        .build())
}

fn physical(bytes: &[u8], guid: u128) -> TestResult<u32> {
    let guid: Guid = common::guid(guid).parse()?;
    let forge = DataForge::open(bytes)?;
    Ok(forge
        .record_by_guid(guid)
        .ok_or("physical fixture Record")?
        .index())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{BuiltCatalog, CatalogError, CatalogPublicationMode};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::DcbOrigin;
use crate::manufacturer_v3_fixture::{self as fixture, Slot, State, TestResult};

#[test]
fn every_owned_parent_and_leaf_preserves_its_actual_source_state() -> TestResult {
    for slot in [
        Slot::Code,
        Slot::Localization,
        Slot::Name,
        Slot::Description,
    ] {
        for state in [
            State::Absent,
            State::Null,
            State::Empty,
            State::Present,
            State::Invalid,
            State::Unreadable,
        ] {
            assert_source_state(slot, state)?;
        }
    }
    Ok(())
}

fn assert_source_state(slot: Slot, state: State) -> TestResult {
    let database = fixture::state_database(slot, state);
    let mode = if matches!(state, State::Invalid | State::Unreadable) {
        CatalogPublicationMode::DiagnosticPartial
    } else {
        CatalogPublicationMode::Normal
    };
    let (_, catalog) = fixture::build(&database, DcbOrigin::Plain, mode, None)?;
    let path = match slot {
        Slot::Code => "/code",
        Slot::Localization => "/localization",
        Slot::Name => "/localization/value/name",
        Slot::Description => "/localization/value/description",
    };
    let carrier = fixture::at(fixture::row(&catalog)?, path)?;
    assert_eq!(
        fixture::at(carrier, "/state")?,
        &json!(state.wire()),
        "{slot:?} {state:?}"
    );
    if !matches!(state, State::Present | State::Empty) {
        assert!(
            fixture::at(carrier, "/value")?.is_null(),
            "non-present source values retain required null"
        );
    }
    if matches!(state, State::Invalid | State::Unreadable) {
        let normal = fixture::build(
            &database,
            DcbOrigin::Plain,
            CatalogPublicationMode::Normal,
            None,
        );
        assert_normal_rejects(normal)?;
    }
    Ok(())
}

#[test]
fn an_authored_empty_locale_is_a_present_key_and_is_counted_per_lookup() -> TestResult {
    let (built, catalog) = fixture::normal(&fixture::state_database(Slot::Name, State::Empty))?;
    let name = fixture::text(&catalog, "publicNames", "name")?;
    assert_eq!(fixture::at(name, "/key/state")?, &json!("present"));
    assert_eq!(fixture::at(name, "/key/value")?, &json!(""));
    assert_eq!(fixture::at(name, "/id/value")?, &json!("name:0:"));
    for language in ["en", "de"] {
        crate::text_forms_assertions::absent(crate::text_forms_assertions::forms(name, language)?)?;
        let summary = built
            .summary()
            .texts()
            .iter()
            .find(|text| text.language.code() == language)
            .ok_or("language summary")?;
        assert_eq!(
            (summary.resolved, summary.unresolved, summary.placeholder),
            (1, 1, 0)
        );
    }
    Ok(())
}

#[test]
fn a_present_empty_localization_object_is_distinct_from_an_absent_parent() -> TestResult {
    let (_, empty) = fixture::normal(&fixture::state_database(Slot::Localization, State::Empty))?;
    let (_, absent) = fixture::normal(&fixture::state_database(Slot::Localization, State::Absent))?;
    assert_eq!(
        fixture::at(fixture::row(&empty)?, "/localization/state")?,
        &json!("present")
    );
    for field in ["name", "description"] {
        assert_eq!(
            fixture::at(
                fixture::row(&empty)?,
                &format!("/localization/value/{field}/state")
            )?,
            &json!("absent")
        );
    }
    assert!(fixture::at(fixture::row(&absent)?, "/localization/value")?.is_null());
    Ok(())
}

fn assert_normal_rejects(result: TestResult<(BuiltCatalog, Value)>) -> TestResult {
    let Err(error) = result else {
        return Err("bad source fields must block normal publication".into());
    };
    assert!(
        matches!(
            error.downcast_ref::<CatalogError>(),
            Some(CatalogError::IncompleteExport(_))
        ),
        "malformed authored fields refuse normal publication with its typed error"
    );
    Ok(())
}

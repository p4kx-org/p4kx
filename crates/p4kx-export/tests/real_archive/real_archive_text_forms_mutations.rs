// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use serde_json::{Value, json};

use super::{Fixture, TestResult};

pub(super) fn wrong_or_missing() -> TestResult {
    let fixture = Fixture::new()?;
    for kind in [Kind::ResourceTypes, Kind::Commodities] {
        let id = super::reference(&fixture.catalog, kind)?;
        for field in ["sourceText", "displayText"] {
            let mut wrong = fixture.catalog.clone();
            super::replace_value(
                super::target(&mut wrong, id)?,
                &format!("/translations/value/en/{field}/value"),
                json!("Wrong source text"),
            )?;
            assert!(
                fixture.coverage(&wrong, kind)?.0.contradictions > 0,
                "each actual form is compared against independent original source lines"
            );
            let mut missing = fixture.catalog.clone();
            let carrier = super::target(&mut missing, id)?
                .pointer_mut(&format!("/translations/value/en/{field}"))
                .ok_or("actual direct English form")?;
            super::replace_value(carrier, "/state", json!("absent"))?;
            super::replace_value(carrier, "/value", Value::Null)?;
            assert!(
                fixture.coverage(&missing, kind)?.0.contradictions > 0,
                "an actual winning source or display form cannot silently disappear"
            );
        }
    }
    Ok(())
}

pub(super) fn require_metadata(fixture: &Fixture) -> TestResult {
    for kind in [Kind::ResourceTypes, Kind::Commodities] {
        let id = super::reference(&fixture.catalog, kind)?;
        for form in ["sourceText", "displayText"] {
            for field in [
                "state",
                "value",
                "containsEncryptedSource",
                "sourceUsesRefs",
                "sourceSetComplete",
            ] {
                let mut missing = fixture.catalog.clone();
                super::target(&mut missing, id)?
                    .pointer_mut(&format!("/translations/value/en/{form}"))
                    .and_then(Value::as_object_mut)
                    .ok_or("actual direct text form")?
                    .remove(field)
                    .ok_or("actual required source metadata")?;
                assert!(
                    fixture.coverage(&missing, kind).is_err(),
                    "each form requires its immediate {field} metadata"
                );
            }
        }
        reject_scalar_group(fixture, kind, id)?;
    }
    Ok(())
}

fn reject_scalar_group(fixture: &Fixture, kind: Kind, id: &str) -> TestResult {
    let mut scalar = fixture.catalog.clone();
    let group = super::target(&mut scalar, id)?
        .pointer_mut("/translations/value/en")
        .ok_or("actual English forms")?;
    *group = group
        .get("displayText")
        .ok_or("actual direct display form")?
        .clone();
    assert!(
        fixture.coverage(&scalar, kind).is_err(),
        "the former scalar language carrier is not a compatibility input"
    );
    Ok(())
}

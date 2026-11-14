// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

#[path = "title_missing_fixture.rs"]
mod fixture;

use self::fixture::Case;
use crate::health_origin_assertions as assertions;
use crate::health_origin_fixture::{Input, TestResult};

#[test]
fn a_terminal_unreadable_title_does_not_certify_complete_contributors() -> TestResult {
    let cases = controlled(Case::Unreadable)?;
    for (input, source) in cases {
        let (_, catalog) = source.diagnostic()?;
        let row = row(&catalog)?;
        for (name, ordinal) in [("title", "0"), ("description", "1")] {
            let text = row.get(name).ok_or("actual projected text")?;
            let cell = assertions::at(text, "/key")?;
            assert_eq!(assertions::at(cell, "/state")?, "unreadable");
            assert_eq!(assertions::at(cell, "/value")?, &Value::Null);
            assert_eq!(assertions::at(text, "/translations")?, &json!({}));
            failed_site(&catalog, cell, ordinal)?;
            assert_eq!(
                assertions::at(cell, "/sourceSetComplete")?,
                false,
                "the terminal failed original read cannot certify the missing value's contributors"
            );
            let expected = matches!(input, Input::Encrypted).then_some(true);
            assert_eq!(
                assertions::at(cell, "/containsEncryptedSource")?,
                &json!(expected),
                "known encrypted selectors survive, while incomplete plain input is unknown"
            );
        }
    }
    Ok(())
}

#[test]
fn readable_missing_title_and_readable_fallback_keep_their_exact_complete_sets() -> TestResult {
    let absent = controlled(Case::Absent)?;
    let fallback = controlled(Case::Fallback)?;
    for (input, source) in absent {
        let (_, catalog) = source.diagnostic()?;
        for name in ["title", "description"] {
            let text = row(&catalog)?.get(name).ok_or("missing text projection")?;
            let cell = assertions::at(text, "/key")?;
            assert_eq!(assertions::at(cell, "/state")?, "absent");
            complete(cell, input)?;
            assert_eq!(assertions::at(text, "/translations")?, &json!({}));
        }
    }
    for (input, source) in fallback {
        let (_, catalog) = source.diagnostic()?;
        let title = assertions::at(row(&catalog)?, "/title")?;
        let key = assertions::at(title, "/key")?;
        assert_eq!(assertions::at(key, "/state")?, "present");
        complete(key, input)?;
        let text = assertions::rows(&catalog, "/texts/gameTexts")?
            .iter()
            .find(|text| text.pointer("/id/value") == key.get("value"))
            .ok_or("actual canonical fallback key")?;
        assert_eq!(assertions::at(text, "/key/value")?, fixture::WINNER);
        let english = assertions::at(title, "/translations/en")?;
        assert_eq!(assertions::at(english, "/value")?, "Readable fallback");
        complete(english, input)?;
    }
    Ok(())
}

fn controlled(case: Case) -> TestResult<Vec<(Input, crate::health_origin_fixture::Fixture)>> {
    [Input::Plain, Input::Encrypted, Input::Standalone]
        .into_iter()
        .map(|input| fixture::open(case, input).map(|source| (input, source)))
        .collect()
}

fn row(catalog: &Value) -> TestResult<&Value> {
    let rows = assertions::rows(catalog, "/projections/missionTitles/rows")?;
    let [row] = rows else {
        return Err("one physical contract title projection".into());
    };
    assert_eq!(
        assertions::at(row, "/contract/recordIndex")?,
        0,
        "the title projection retains its actual Generator owner"
    );
    Ok(row)
}

fn complete(cell: &Value, input: Input) -> TestResult {
    assert_eq!(
        assertions::at(cell, "/sourceSetComplete")?,
        true,
        "the readable result has its complete actually consulted input set"
    );
    assert_eq!(
        assertions::at(cell, "/containsEncryptedSource")?,
        &json!(input.marker()),
        "the direct marker follows the selected original input"
    );
    Ok(())
}

fn failed_site(catalog: &Value, cell: &Value, ordinal: &str) -> TestResult {
    let site = json!(["1", ordinal]);
    let matches_site = |used: &&Value| {
        used.pointer("/location/type") == Some(&json!("subresource"))
            && used.pointer("/location/namespace") == Some(&json!("dataforgeValue"))
            && used.pointer("/location/indices") == Some(&site)
            && used.pointer("/location/field") == Some(&json!("$value"))
    };
    assert!(
        assertions::rows(catalog, "/provenance/uses")?
            .iter()
            .filter(matches_site)
            .any(|used| used.get("role") == Some(&json!("attempted"))),
        "the actually failed original slot remains an independent attempt"
    );
    assert!(
        !assertions::uses(catalog, cell)?.iter().any(matches_site),
        "the failed original payload does not acquire contribution membership"
    );
    Ok(())
}

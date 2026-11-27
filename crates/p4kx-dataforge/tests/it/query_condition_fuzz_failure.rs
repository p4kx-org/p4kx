// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::query::{Comparison, Condition, LiteralValue, Query, SelectionError};
use p4kx_dataforge::{DataForge, Record, Value, ValueError};

use super::TestResult;

/// Checks that a fuzz selection failure names an admitted original raw-field comparison.
pub(super) fn check(
    forge: &DataForge<'_>,
    query: &Query,
    error: &SelectionError,
    text: &str,
) -> TestResult {
    let record = forge
        .record(error.record_index())
        .ok_or("error names no Record")?;
    admitted(&record, query)?;
    compares_raw(query, error.comparison_index())?;
    assert!(
        matches!(
            error.value_error(),
            ValueError::InvalidString {
                offset: u32::MAX,
                ..
            }
        ),
        "{text}: {error:?}"
    );
    assert!(record.name()?.starts_with("Gadget."), "{text}: {error:?}");
    assert_eq!(
        record.record_path()?,
        error.record_path(),
        "the error path belongs to its Record"
    );
    let tree = record.tree()?;
    assert_eq!(
        tree.root().field("raw"),
        Some(&Value::Error(*error.value_error())),
        "the actual original raw field has exactly the reported failure"
    );
    Ok(())
}

fn admitted(record: &Record<'_>, query: &Query) -> TestResult {
    let source_type = record.type_name()?;
    if source_type != "Gadget"
        || query
            .from()
            .is_some_and(|name| name.as_str() != source_type)
    {
        return Err(
            "the failed original Gadget must be admitted by this Query's from clause".into(),
        );
    }
    if let Some(glob) = query.at()
        && (glob.as_str() != "libs" || !record.record_path()?.starts_with("libs/"))
    {
        return Err(
            "the failed original Gadget must be inside the fuzz Query's actual path scope".into(),
        );
    }
    Ok(())
}

fn compares_raw(query: &Query, index: usize) -> TestResult {
    let mut remaining = index;
    let comparison = query
        .condition()
        .and_then(|condition| comparison_at(condition, &mut remaining))
        .ok_or("the failure comparison index must exist in the parsed condition")?;
    if comparison.path().head().as_str() != "raw"
        || !comparison.path().tail().is_empty()
        || !matches!(comparison.literal().value(), LiteralValue::Text(_))
    {
        return Err("the failure must name the simple original raw String comparison".into());
    }
    Ok(())
}

fn comparison_at<'a>(condition: &'a Condition, remaining: &mut usize) -> Option<&'a Comparison> {
    match condition {
        Condition::Compare(comparison) if *remaining == 0 => Some(comparison),
        Condition::Compare(_) => {
            *remaining -= 1;
            None
        }
        Condition::Not(inner) => comparison_at(inner, remaining),
        Condition::And(parts) | Condition::Or(parts) => {
            parts.iter().find_map(|part| comparison_at(part, remaining))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::query_condition_fuzz;
    use p4kx_dataforge::DataForge;
    use p4kx_dataforge::query::Query;

    use super::TestResult;

    #[test]
    fn nonraw_seed_controls_and_decisive_proofs_reject_selection_errors() -> TestResult {
        let bytes = query_condition_fuzz::database();
        let forge = DataForge::open(&bytes)?;
        for (text, count) in [
            ("from Item where mass > 0", 43),
            ("from Gadget where name >= 0", 64),
            ("at \"libs\" where name = \"Name 1\"", 1),
            ("from Gadget where name >= 0 or raw = \"ok\"", 64),
            ("from Gadget where raw = \"ok\" or name >= 0", 64),
            ("from Gadget where name < 0 and raw = \"ok\"", 0),
            ("from Gadget where raw = \"ok\" and name < 0", 0),
        ] {
            let compiled = Query::parse(text)?.compile(&forge)?;
            let run = compiled.run()?;
            assert_eq!(
                run.len(),
                count,
                "the actual fixture gives a complete result: {text}"
            );
        }
        Ok(())
    }

    #[test]
    fn a_real_raw_failure_does_not_authorize_an_unrelated_query_or_comparison() -> TestResult {
        let bytes = query_condition_fuzz::database();
        let forge = DataForge::open(&bytes)?;
        for (text, ordinal) in [
            ("from Gadget where name < 0 or raw = \"ok\"", 1),
            (
                "from Gadget where not (name >= 0) or name < 0 or raw = \"ok\"",
                2,
            ),
        ] {
            let query = Query::parse(text)?;
            let compiled = query.compile(&forge)?;
            let error = compiled
                .run()
                .err()
                .ok_or("the demanded original raw value is invalid")?;
            assert_eq!(
                error.comparison_index(),
                ordinal,
                "actual parsed condition order"
            );
            super::check(&forge, &query, &error, text)?;
            for unrelated in [
                "from Item where mass > 0",
                "from Gadget where name >= 0",
                "from Gadget where raw = \"ok\" or name >= 0",
                "from Gadget at \"elsewhere\" where name < 0 or raw = \"ok\"",
            ] {
                assert!(
                    super::check(&forge, &Query::parse(unrelated)?, &error, unrelated).is_err(),
                    "a real raw failure cannot satisfy an unrelated Query oracle: {unrelated}"
                );
            }
        }
        Ok(())
    }
}

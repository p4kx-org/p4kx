// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::Serialize;
use serde_json::{Map, Value};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Reads the actual JSON wire representation of a serializable test value.
pub(crate) fn canonical_value<T: Serialize + ?Sized>(value: &T) -> TestResult<Value> {
    Ok(serde_json::from_slice(&serde_json::to_vec(value)?)?)
}

/// Checks every JSON field and reports the first differing RFC6901 path without a payload dump.
pub(crate) fn assert_canonical<T: Serialize + ?Sized>(
    actual: &T,
    expected: &Value,
    subject: &str,
) -> TestResult {
    let actual = canonical_value(actual)?;
    if let Some(difference) = first_difference(&actual, expected, &mut String::new()) {
        return Err(format!(
            "{subject} differs at {:?}: {}",
            difference.pointer, difference.description
        )
        .into());
    }
    Ok(())
}

struct Difference {
    pointer: String,
    description: &'static str,
}

fn first_difference(actual: &Value, expected: &Value, path: &mut String) -> Option<Difference> {
    if actual == expected {
        return None;
    }
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => {
            object_difference(actual, expected, path)
        }
        (Value::Array(actual), Value::Array(expected)) => array_difference(actual, expected, path),
        _ => Some(Difference {
            pointer: path.clone(),
            description: "scalar value or JSON type differs",
        }),
    }
}

fn object_difference(
    actual: &Map<String, Value>,
    expected: &Map<String, Value>,
    path: &mut String,
) -> Option<Difference> {
    for (key, value) in actual {
        let position = path.len();
        push_key(path, key);
        let Some(expected) = expected.get(key) else {
            return Some(Difference {
                pointer: path.clone(),
                description: "unexpected object field",
            });
        };
        if let Some(found) = first_difference(value, expected, path) {
            return Some(found);
        }
        path.truncate(position);
    }
    for key in expected.keys() {
        if !actual.contains_key(key) {
            push_key(path, key);
            return Some(Difference {
                pointer: path.clone(),
                description: "missing object field",
            });
        }
    }
    None
}

fn array_difference(actual: &[Value], expected: &[Value], path: &mut String) -> Option<Difference> {
    if actual.len() != expected.len() {
        return Some(Difference {
            pointer: path.clone(),
            description: "array length differs",
        });
    }
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        let position = path.len();
        path.push('/');
        path.push_str(&index.to_string());
        if let Some(found) = first_difference(actual, expected, path) {
            return Some(found);
        }
        path.truncate(position);
    }
    None
}

fn push_key(path: &mut String, key: &str) {
    path.push('/');
    for value in key.chars() {
        match value {
            '~' => path.push_str("~0"),
            '/' => path.push_str("~1"),
            value => path.push(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TestResult, assert_canonical, canonical_value};
    use serde::Serialize;
    use serde_json::json;

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Single {
        raw_bits: &'static str,
        finite: f32,
    }

    #[test]
    fn single_wire_round_trip_matches_the_writer_without_ignoring_original_bits() -> TestResult {
        let source = Single {
            raw_bits: "3f59999a",
            finite: 0.85,
        };
        let expected = json!({"rawBits":"3f59999a", "finite":0.85});
        assert_canonical(&source, &expected, "source Single")?;
        assert_eq!(
            canonical_value(&0.85_f32)?,
            json!(0.85),
            "Single decimals follow the actual JSON writer"
        );
        let wrong_bits = Single {
            raw_bits: "3f59999b",
            finite: 0.85,
        };
        let error = assert_canonical(&wrong_bits, &expected, "wrong original bits")
            .err()
            .ok_or("wrong bits must fail")?;
        assert!(
            error.to_string().contains("/rawBits"),
            "Exact original bits remain part of exhaustive comparison"
        );
        Ok(())
    }

    #[test]
    fn wide_integers_order_types_and_missing_fields_are_compared_exactly() -> TestResult {
        assert_canonical(&u64::MAX, &json!(u64::MAX), "wide integer")?;
        for (actual, expected, pointer) in [
            (
                json!({"v":9_007_199_254_740_993_u64}),
                json!({"v":9_007_199_254_740_992_u64}),
                "/v",
            ),
            (json!([1, 2]), json!([2, 1]), "/0"),
            (json!({"v":1}), json!({"v":"1"}), "/v"),
            (json!({}), json!({"v":1}), "/v"),
            (json!({"extra":1}), json!({}), "/extra"),
            (json!({"/~":[1]}), json!({"/~":[2]}), "/~1~0/0"),
        ] {
            let error = assert_canonical(&actual, &expected, "counterfixture")
                .err()
                .ok_or("difference must fail")?;
            assert!(
                error.to_string().contains(pointer),
                "The first exact mismatch retains its escaped JSON path"
            );
        }
        Ok(())
    }
}

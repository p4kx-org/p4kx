// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::compare_rows::Projection;
use super::measurement;
use super::selected_origins::SelectedOrigins;
use super::{BoundMeasurement, Origins, TestResult, artifact, first_difference};

impl BoundMeasurement {
    pub(crate) fn verify_selected_row(
        &self,
        path: &str,
        expected: &Value,
        origins: &Origins,
        same_identity: impl Fn(&Value) -> bool,
    ) -> TestResult {
        self.verify_selected_scope(&[path], origins, |catalog| {
            check_selected_row(catalog, path, expected, same_identity)
        })
    }

    pub(crate) fn verify_selected_scope(
        &self,
        paths: &[&str],
        origins: &Origins,
        verify: impl FnOnce(&Value) -> TestResult,
    ) -> TestResult {
        self.watch.check()?;
        let mut capture = measurement::normal_capture();
        for path in paths {
            capture.keep(path);
        }
        for (section, expected) in origins.inventories() {
            capture.project(section, Projection::Origins(SelectedOrigins(expected)));
        }
        let catalog = artifact::read_completed(&self.directory, |input| Ok(capture.read(input)?))?;
        self.binding.check_catalog(&catalog)?;
        verify(&catalog)?;
        origins.verify(&catalog)?;
        self.watch.check()?;
        Ok(())
    }
}

pub(crate) fn check_selected_row(
    catalog: &Value,
    path: &str,
    expected: &Value,
    same_identity: impl Fn(&Value) -> bool,
) -> TestResult {
    let rows = catalog
        .pointer(path)
        .and_then(Value::as_array)
        .ok_or("the Catalog is missing the selected row array")?;
    let mut matches = rows.iter().filter(|row| same_identity(row));
    let row = matches
        .next()
        .ok_or("the Catalog is missing the selected row")?;
    if matches.next().is_some() {
        return Err("the Catalog contains more than one row with the selected identity".into());
    }
    if let Some(field) = first_difference(expected, row) {
        return Err(
            format!("Catalog and SDK payload or provenance differ at {path}[]{field}").into(),
        );
    }
    Ok(())
}

#[test]
fn selected_catalog_row_requires_one_identical_match() -> TestResult {
    use serde_json::json;

    let expected = json!({"key": {"value": "selected"}, "inventory": {"value": "expected"}});
    let unrelated = json!({"key": {"value": "other"}});
    let same_identity =
        |row: &Value| row.pointer("/key/value").and_then(Value::as_str) == Some("selected");
    let correct = json!({"shops": [unrelated, expected]});
    check_selected_row(&correct, "/shops", &expected, same_identity)?;

    for rows in [
        json!([unrelated]),
        json!([expected, expected]),
        json!([{"key": {"value": "selected"}, "inventory": {"value": "changed"}}]),
    ] {
        assert!(
            check_selected_row(&json!({"shops": rows}), "/shops", &expected, same_identity)
                .is_err(),
            "the nominated row must exist exactly once with the expected values"
        );
    }
    Ok(())
}

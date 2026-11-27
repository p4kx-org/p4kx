// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde_json::{Value, json};

use super::{TestResult, row_schema};
use crate::schema_navigation;

pub(super) struct Route {
    pub(super) dataset: &'static str,
    pub(super) schema: Value,
    pub(super) remaining: Option<&'static str>,
}

impl Route {
    pub(super) fn queried_value<Item: JsonSchema + serde::Serialize>(
        dataset: &'static str,
        value: &Item,
        remaining: &'static str,
    ) -> TestResult<Self> {
        Self::queried_rows(dataset, std::slice::from_ref(value), remaining)
    }

    pub(super) fn queried_rows<Row: JsonSchema + serde::Serialize>(
        dataset: &'static str,
        rows: &[Row],
        remaining: impl Into<Option<&'static str>>,
    ) -> TestResult<Self> {
        if rows.is_empty() {
            return Err(format!("SDK witness returned no row for {dataset}").into());
        }
        Ok(Self::with_row_schema::<Row>(dataset, remaining.into()))
    }

    pub(super) fn queried_uninhabited_rows<Row: JsonSchema + serde::Serialize>(
        dataset: &'static str,
        rows: &[Row],
        remaining: &'static str,
    ) -> TestResult<Self> {
        let route = Self::with_row_schema::<Row>(dataset, Some(remaining));
        let row = schema_navigation::schema_at(&route.schema, "/[]")?;
        if !rows.is_empty() || (row != &Value::Bool(false) && row != &json!({"not": {}})) {
            return Err(format!(
                "SDK availability witness requires an uninhabited row schema for {dataset}"
            )
            .into());
        }
        Ok(route)
    }

    fn with_row_schema<Row: JsonSchema + serde::Serialize>(
        dataset: &'static str,
        remaining: Option<&'static str>,
    ) -> Self {
        Self {
            dataset,
            schema: schemars::generate::SchemaSettings::draft2020_12()
                .for_serialize()
                .with_transform(row_schema::remove_descriptions)
                .into_generator()
                .into_root_schema_for::<Vec<Row>>()
                .to_value(),
            remaining,
        }
    }
}

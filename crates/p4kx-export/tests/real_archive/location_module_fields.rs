// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value as Original};
use serde_json::Value;

use super::TestResult;
use super::fields::Check;
use crate::authored_definition_witness::{field, verify_array};
use crate::catalog_v3_consumer as consumer;
use crate::original_item_arena as arena;

pub(super) fn richness(source: Option<&Original<'_>>) -> usize {
    let Some(Original::Array(modules)) = source else {
        return 0;
    };
    modules.len()
        + modules
            .iter()
            .map(|item| match item {
                Original::Struct(module) => ["missionInputs", "properties"]
                    .into_iter()
                    .map(|name| match module.field(name) {
                        Some(Original::Array(items)) => items.len(),
                        _ => 0,
                    })
                    .sum(),
                _ => 0,
            })
            .sum::<usize>()
}

pub(super) fn authored_crew(source: Option<&Original<'_>>) -> usize {
    let mut pending: Vec<_> = source.into_iter().collect();
    let mut found = 0;
    while let Some(value) = pending.pop() {
        match value {
            Original::Struct(original) => {
                if original.type_name() == "SpawnDescription_Ship"
                    && matches!(
                        original.field("crewInheritsLocationFaction"),
                        Some(Original::Bool(_))
                    )
                {
                    found += 1;
                }
                pending.extend(original.fields().iter().map(p4kx_extraction::Field::value));
            }
            Original::Array(values) => pending.extend(values.iter()),
            _ => {}
        }
    }
    found
}

impl<'source> Check<'_, 'source> {
    pub(super) fn mission_module(&self, source: &StructValue<'source>, row: &Value) -> TestResult {
        self.scalar(source, row, "isPersistent")?;
        self.scalar(source, row, "missionModule")?;
        self.array(source, row, "missionInputs", Self::mission_input)?;
        self.array(source, row, "properties", Self::mission_property)
    }

    fn mission_input(&self, source: &StructValue<'source>, row: &Value) -> TestResult {
        let source_type = source.type_name();
        assert!(
            consumer::present(field(row, "sourceType")?)?.as_str() == Some(source_type),
            "the module input retains its concrete original type"
        );
        self.scalar(source, row, "name")?;
        let kind = match source_type {
            "MissionInitParamBoolean" => "boolean",
            "MissionInitParamInteger" => "integer",
            "MissionInitParamFloat" => "float",
            "MissionInitParamActivity" => "activity",
            "MissionInitParamTag" => "tag",
            "AbstractMissionInitParam" => "base",
            _ => "authored",
        };
        assert!(
            field(row, "kind")?.as_str() == Some(kind),
            "the module input retains its actual interpretation"
        );
        match kind {
            "boolean" | "integer" | "activity" => self.scalar(source, row, "value"),
            "float" => self.single(source, row, "value"),
            "tag" => self.module_input_tags(source.field("value"), field(row, "value")?),
            _ => self.compare_authored(source, field(row, "authoredFields")?, &["name"]),
        }
    }

    fn module_input_tags(&self, source: Option<&Original<'source>>, row: &Value) -> TestResult {
        let Some(original @ Original::Struct(term)) = source else {
            return self.state(source, row);
        };
        self.origin(row, original, "$type")?;
        let row = consumer::present(row)?;
        for name in ["positiveTags", "negativeTags"] {
            let source = term.field(name);
            let row = field(row, name)?;
            if let Some(source) = source {
                self.origin(row, source, "$value")?;
            }
            verify_array(source, row, |item, cell| self.link(item, cell, "Tag"))?;
        }
        Ok(())
    }

    fn mission_property(&self, source: &StructValue<'source>, row: &Value) -> TestResult {
        self.scalar(source, row, "missionVariableName")?;
        self.scalar(source, row, "extendedTextToken")?;
        let raw = source.field("value");
        let row = field(row, "value")?;
        let Some(original @ Original::Struct(body)) = raw else {
            return self.state(raw, row);
        };
        self.origin(row, original, "$type")?;
        let row = consumer::present(row)?;
        let source_type = field(row, "sourceType")?;
        assert!(
            consumer::present(source_type)?.as_str() == Some(body.type_name()),
            "the authored property retains its concrete type"
        );
        self.origin(source_type, original, "$type")?;
        self.compare_authored(body, field(row, "authoredFields")?, &[])
    }

    fn compare_authored(
        &self,
        source: &StructValue<'source>,
        authored: &Value,
        excluded: &[&str],
    ) -> TestResult {
        let fields = source
            .fields()
            .iter()
            .filter(|field| !excluded.contains(&field.name()));
        arena::compare(self.trace, fields, authored)?;
        Ok(())
    }
}

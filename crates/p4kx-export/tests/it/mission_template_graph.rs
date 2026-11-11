// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::flow_owner_graph::Owner;
use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::{BrokerContext, Context, Template};

/// Borrowed navigation to one selected canonical property and its separate interpretation.
pub(crate) struct Property<'catalog> {
    pub(crate) owner: Owner<'catalog>,
    pub(crate) projection: &'catalog Value,
    pub(crate) selected: &'catalog Value,
    pub(crate) node: &'catalog Value,
    pub(crate) meaning: &'catalog Value,
}

impl<'catalog> Property<'catalog> {
    pub(crate) fn new(catalog: &'catalog Value, variable: &str) -> TestResult<Self> {
        let guid = crate::catalog_crafting_support::text(crate::scoped_fixture::TEMPLATE);
        Self::from_context(&Template::new(catalog, &guid)?, variable)
    }

    pub(crate) fn broker(catalog: &'catalog Value, variable: &str) -> TestResult<Self> {
        let guid = crate::health_origin_fixture::guid(crate::scoped_fixture::BROKER);
        Self::from_context(&BrokerContext::new(catalog, &guid)?, variable)
    }

    fn from_context(graph: &dyn Context<'catalog>, variable: &str) -> TestResult<Self> {
        let projection = graph.projection();
        let mut found = None;
        for property in raw::array(projection, "properties")? {
            let binding = property
                .get("variable")
                .ok_or("actual variable declaration binding")?;
            if binding.get("state") != Some(&json!("present")) {
                continue;
            }
            let field = binding
                .get("field")
                .ok_or("actual selected variable field")?;
            let value = graph.bound(field)?;
            match value.pointer("/value/state").and_then(Value::as_str) {
                Some("null" | "unreadable") => continue,
                Some("present") => {}
                _ => return Err("actual raw variable cell state".into()),
            }
            if text(value)? != variable {
                continue;
            }
            assert!(
                found.is_none(),
                "the controlled variable selects one physical property"
            );
            found = Some(property);
        }
        let selected = found.ok_or_else(|| format!("canonical property {variable}"))?;
        let reference = selected
            .get("value")
            .ok_or("required nullable property value binding")?;
        let owner = graph.owner();
        let node = owner.follow(reference)?;
        let meaning = unique(
            raw::array(projection, "valueMeanings")?,
            "source",
            reference,
        )?;
        Ok(Self {
            owner: Owner {
                source: owner.source,
                values: owner.values,
            },
            projection,
            selected,
            node,
            meaning,
        })
    }

    pub(crate) fn field(&self, node: &Value, name: &str) -> TestResult<&'catalog Value> {
        Context::field(self, node, name)
    }

    pub(crate) fn bound(&self, reference: &Value) -> TestResult<&'catalog Value> {
        Context::bound(self, reference)
    }

    pub(crate) fn element(&self, node: &Value, index: usize) -> TestResult<&'catalog Value> {
        let reference = raw::elements(node)?
            .get(index)
            .ok_or("actual original array slot")?;
        self.owner.follow(reference)
    }

    pub(crate) fn kind(&self, expected: &str) -> TestResult {
        assert_eq!(
            self.selected.get("valueKind"),
            Some(&json!(expected)),
            "selected value has its own contextual kind"
        );
        assert_eq!(
            raw::present(self.meaning.get("kind").ok_or("sourced property kind")?)?,
            &json!(expected),
            "required interpretation retains that exact source kind"
        );
        assert_eq!(
            self.meaning.get("status"),
            Some(&json!("recognized")),
            "the complete known source is recognized"
        );
        Ok(())
    }

    pub(crate) fn filter_kind(&self, node: &Value, expected: &str) -> TestResult {
        let row = self.context("scopedFilters", "source", node)?;
        assert_eq!(
            raw::present(row.get("kind").ok_or("actual filter kind")?)?,
            &json!(expected),
            "the actual selected filter has its own contextual operation"
        );
        Ok(())
    }

    pub(crate) fn binding(&self, node: &Value) -> TestResult<&'catalog Value> {
        let reference = self
            .owner
            .reference(crate::flow_owner_graph::ordinal(node)?)?;
        unique(
            raw::array(self.projection, "propertyBindings")?,
            "caller",
            &reference,
        )
    }

    fn context(&self, table: &str, key: &str, node: &Value) -> TestResult<&'catalog Value> {
        let reference = self
            .owner
            .reference(crate::flow_owner_graph::ordinal(node)?)?;
        unique(raw::array(self.meaning, table)?, key, &reference)
    }
}

impl<'catalog> Context<'catalog> for Property<'catalog> {
    fn owner(&self) -> &Owner<'catalog> {
        &self.owner
    }
    fn projection(&self) -> &'catalog Value {
        self.projection
    }
}

fn unique<'value>(
    rows: &'value [Value],
    key: &str,
    reference: &Value,
) -> TestResult<&'value Value> {
    let mut found = None;
    for row in rows.iter().filter(|row| row.get(key) == Some(reference)) {
        assert!(
            found.is_none(),
            "one interpretation for the actual owner-local occurrence"
        );
        found = Some(row);
    }
    found.ok_or_else(|| "actual contextual occurrence binding".into())
}

pub(crate) fn text(node: &Value) -> TestResult<&str> {
    let payload = raw::present(node.get("value").ok_or("immediate original text value")?)?;
    assert!(
        matches!(
            payload.get("type").and_then(Value::as_str),
            Some("string" | "locale")
        ),
        "the original string or Locale stays typed"
    );
    payload
        .pointer("/value/sourceText")
        .and_then(Value::as_str)
        .ok_or_else(|| "complete original text, independent of its protection domain".into())
}

pub(crate) fn scalar<'node>(node: &'node Value, kind: &str) -> TestResult<&'node Value> {
    raw::payload(node, kind)?
        .get("value")
        .ok_or_else(|| "typed original scalar contents".into())
}

pub(crate) fn reference(node: &Value, guid: &str, index: Option<u32>) -> TestResult {
    if let Some(index) = index {
        let target = scalar(node, "recordReference")?;
        assert_eq!(
            target.get("status"),
            Some(&json!("present")),
            "actual source target is physically present"
        );
        for (field, expected) in [("guid", json!(guid)), ("targetRecordIndex", json!(index))] {
            assert_eq!(
                raw::present(
                    target
                        .get(field)
                        .ok_or("independently sourced target datum")?
                )?,
                &expected,
                "the original target identity remains exact"
            );
        }
        assert!(
            raw::present(target.get("recordPath").ok_or("actual target path")?)?.is_string(),
            "a present physical target retains its actual source path"
        );
        assert!(
            raw::present(target.get("targetType").ok_or("actual target type")?)?.is_string(),
            "a present physical target retains its actual source type"
        );
    } else {
        assert_eq!(
            node.pointer("/value/state"),
            Some(&json!("unreadable")),
            "the original dangling Reference has an exact decoder failure"
        );
        assert_eq!(
            node.pointer("/value/value"),
            Some(&Value::Null),
            "no fabricated successful target payload"
        );
        let error = node
            .get("originalError")
            .ok_or("actual original Reference failure")?;
        assert_eq!(
            error.get("type"),
            Some(&json!("danglingReference")),
            "the actual failure category remains typed"
        );
        assert_eq!(
            raw::present(error.get("guid").ok_or("actual authored dangling GUID")?)?,
            &json!(guid),
            "the authored missing identity is preserved instead of resolved by name"
        );
        assert!(
            error.get("targetRecordIndex").is_none() && error.get("recordPath").is_none(),
            "unavailable physical target facts are not fabricated"
        );
    }
    Ok(())
}

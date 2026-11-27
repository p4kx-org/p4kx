// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::TestResult;

pub(super) enum Datum {
    Header,
    Surface(u64),
    Reference(&'static str, u64),
    Root(&'static str, Option<u64>),
}

pub(super) enum DatumLocation {
    Header,
    Item(usize),
}

impl Datum {
    pub(super) fn matched_index(&self, row: &Value) -> TestResult<Option<DatumLocation>> {
        let variant = row.pointer("/data/record_type").and_then(Value::as_str);
        if matches!(self, Self::Header) {
            return Ok(matches!(variant, Some("library-header" | "failure"))
                .then_some(DatumLocation::Header));
        }
        let Some(items) = row.pointer("/data/payload/items").and_then(Value::as_array) else {
            return Ok(None);
        };
        let first = row
            .pointer("/subresource/indices/0")
            .and_then(Value::as_u64)
            .ok_or("batch start")?;
        for (local, value) in items.iter().enumerate() {
            let ordinal = first
                .checked_add(u64::try_from(local)?)
                .ok_or("global position")?;
            if self.matches_item(variant, ordinal, value) {
                return Ok(Some(DatumLocation::Item(local)));
            }
        }
        Ok(None)
    }

    fn matches_item(&self, variant: Option<&str>, ordinal: u64, value: &Value) -> bool {
        match *self {
            Self::Header => false,
            Self::Surface(index) => {
                variant == Some("part")
                    && value.get("type").and_then(Value::as_str) == Some("surface")
                    && value.get("index").and_then(Value::as_u64) == Some(index)
            }
            Self::Reference(kind, index) => variant == Some(kind) && ordinal == index,
            Self::Root(kind, position) => {
                variant == Some("part")
                    && value.get("type").and_then(Value::as_str) == Some("element")
                    && value.pointer("/tree/type").and_then(Value::as_str) == Some("library")
                    && value.pointer("/part/element").and_then(Value::as_u64) == Some(0)
                    && value.pointer("/part/row/type").and_then(Value::as_str) == Some(kind)
                    && value.pointer("/part/row/position").and_then(Value::as_u64) == position
            }
        }
    }
}

pub(super) fn datum(path: &str) -> TestResult<(Datum, String)> {
    let root = "/data/payload";
    let Some(path) = path.strip_prefix(root) else {
        return Ok((Datum::Header, path.to_owned()));
    };
    if let Some(rest) = path.strip_prefix("/materials/") {
        let (position, field) = rest.split_once('/').ok_or("surface field")?;
        return Ok((Datum::Surface(position.parse()?), format!("{root}/{field}")));
    }
    for (legacy, kind) in [
        ("references", "reference"),
        ("shader_references", "shader-reference"),
    ] {
        if let Some(rest) = path.strip_prefix(&format!("/{legacy}/")) {
            let (position, field) = rest.split_once('/').unwrap_or((rest, ""));
            let suffix = if field.is_empty() {
                String::new()
            } else {
                format!("/{field}")
            };
            return Ok((
                Datum::Reference(kind, position.parse()?),
                format!("{root}{suffix}"),
            ));
        }
    }
    if let Some(rest) = path.strip_prefix("/authored/root/") {
        if let Some(rest) = rest.strip_prefix("syntax/attributes/") {
            let (position, field) = rest.split_once('/').ok_or("XML declaration field")?;
            return Ok((
                Datum::Root("declaration", Some(position.parse()?)),
                format!("{root}/part/row/value/{field}"),
            ));
        }
        return Ok((
            Datum::Root("element", None),
            format!("{root}/part/row/{rest}"),
        ));
    }
    Ok((Datum::Header, format!("{root}{path}")))
}

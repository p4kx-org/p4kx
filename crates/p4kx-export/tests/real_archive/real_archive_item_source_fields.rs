// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::{ItemPresentationIndex, TestResult, catalog_v3_consumer};

impl<'catalog> ItemPresentationIndex<'catalog> {
    pub(crate) fn attachment(&self, item: &Value) -> TestResult<Option<&'catalog Value>> {
        self.selected(item, "attachment", "attachmentComponentOrdinal")
    }

    pub(crate) fn item_type(&self, item: &Value) -> TestResult<Option<&'catalog str>> {
        self.attachment_field(item, "itemType")?
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| "original Item type label".into())
            })
            .transpose()
    }

    pub(crate) fn grade(&self, item: &Value) -> TestResult<Option<i64>> {
        self.attachment_field(item, "grade")?
            .map(integer)
            .transpose()
            .map(Option::flatten)
    }

    pub(crate) fn mass(&self, item: &Value) -> TestResult<Option<&'catalog Value>> {
        let Some(physics) = self.selected(item, "physics", "physicsComponentOrdinal")? else {
            return Ok(None);
        };
        let carrier = physics.get("mass").ok_or("original mass carrier")?;
        let Some(number) = catalog_v3_consumer::source_value(carrier)? else {
            return Ok(None);
        };
        let finite = number
            .get("finite")
            .ok_or("original finite mass projection")?;
        if finite.is_null() {
            return Ok(None);
        }
        if !finite.is_number() {
            return Err("original finite mass must be numeric".into());
        }
        Ok(Some(finite))
    }

    pub(crate) fn manufacturer(&self, item: &Value) -> TestResult<Option<&'catalog str>> {
        let Some(attachment) = self.attachment(item)? else {
            return Ok(None);
        };
        let source = attachment
            .get("manufacturer")
            .ok_or("original manufacturer storage")?;
        match source.get("kind").and_then(Value::as_str) {
            Some("authored") => Ok(None),
            Some("record") => {
                let carrier = source
                    .get("value")
                    .ok_or("original manufacturer Reference")?;
                let Some(reference) = catalog_v3_consumer::source_value(carrier)? else {
                    return Ok(None);
                };
                let guid = reference.get("guid").ok_or("authored manufacturer GUID")?;
                catalog_v3_consumer::source_value(guid)?
                    .map(|guid| {
                        guid.as_str()
                            .ok_or_else(|| "authored manufacturer GUID string".into())
                    })
                    .transpose()
            }
            _ => Err("unknown original manufacturer storage".into()),
        }
    }

    fn attachment_field(&self, item: &Value, field: &str) -> TestResult<Option<&'catalog Value>> {
        self.attachment(item)?.map_or(Ok(None), |attachment| {
            let carrier = attachment.get(field).ok_or("original Attachment field")?;
            catalog_v3_consumer::source_value(carrier)
        })
    }

    fn selected(
        &self,
        item: &Value,
        kind: &str,
        selection: &str,
    ) -> TestResult<Option<&'catalog Value>> {
        let carrier = item
            .get("selection")
            .and_then(|selection_map| selection_map.get(selection))
            .ok_or("immediate first physical Item component selection")?;
        let Some(ordinal) = catalog_v3_consumer::source_value(carrier)? else {
            return Ok(None);
        };
        if !ordinal.is_u64() {
            return Err("physical Item component ordinal".into());
        }
        let mut matching = self
            .components(item)?
            .iter()
            .filter(|component| component.pointer("/componentOrdinal/value") == Some(ordinal));
        let component = matching
            .next()
            .ok_or("exact selected physical Item component")?;
        if matching.next().is_some() || component.get("kind").and_then(Value::as_str) != Some(kind)
        {
            return Err("the selected physical Item component has another identity".into());
        }
        catalog_v3_consumer::source_value(
            component
                .get("value")
                .ok_or("selected Item component body")?,
        )
    }
}

fn integer(number: &Value) -> TestResult<Option<i64>> {
    let value = number.get("value");
    match number.get("type").and_then(Value::as_str) {
        Some("int8" | "int16" | "int32" | "uint8" | "uint16" | "uint32") => value
            .and_then(Value::as_i64)
            .map(Some)
            .ok_or_else(|| "exact authored narrow integer".into()),
        Some("int64" | "uint64") => {
            let digits = value
                .and_then(Value::as_str)
                .ok_or("exact wide integer digits")?;
            match digits.parse::<i64>() {
                Ok(value) => Ok(Some(value)),
                Err(_)
                    if number.get("type").and_then(Value::as_str) == Some("uint64")
                        && digits.parse::<u64>().is_ok() =>
                {
                    Ok(None)
                }
                Err(error) => Err(error.into()),
            }
        }
        Some("f32" | "f64") => Ok(None),
        _ => Err("authored numeric kind".into()),
    }
}

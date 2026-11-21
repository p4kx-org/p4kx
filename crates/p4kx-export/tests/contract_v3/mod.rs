// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::error::Error;

use serde_json::Value;

type TestResult<T> = Result<T, Box<dyn Error>>;

pub(crate) fn rows(catalog: &Value) -> TestResult<&[Value]> {
    if catalog.get("contracts").is_some() {
        return Err("contracts must appear only in the v3 projection".into());
    }
    array(catalog, "/projections/contracts/rows")
}

pub(crate) fn identity(row: &Value) -> TestResult<(&str, Option<&str>)> {
    let guid = string(cell(row, "guid")?.ok_or("present contract GUID")?)?;
    let sub = cell(row, "subContractGuid")?.map(string).transpose()?;
    Ok((guid, sub))
}

type Owner<'a> = (&'a str, u64);

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Occurrence<'a> {
    owner: Owner<'a>,
    handler: &'a str,
    contract: &'a str,
    sub: Option<&'a str>,
}

impl<'a> Occurrence<'a> {
    fn read(source: &'a Value) -> TestResult<Self> {
        let sub = member(source, "subContract")?;
        Ok(Self {
            owner: (
                string(member(source, "inputRef")?)?,
                index(member(source, "recordIndex")?)?,
            ),
            handler: value_index(member(source, "handler")?)?,
            contract: value_index(member(source, "contract")?)?,
            sub: (!sub.is_null()).then(|| value_index(sub)).transpose()?,
        })
    }
}

pub(crate) struct Projections<'a> {
    definitions: BTreeMap<Owner<'a>, &'a Value>,
    titles: BTreeMap<Occurrence<'a>, &'a Value>,
    keys: BTreeMap<&'a str, &'a Value>,
}

impl<'a> Projections<'a> {
    pub(crate) fn read(catalog: &'a Value) -> TestResult<Self> {
        let mut definitions = BTreeMap::new();
        for row in array(catalog, "/definitions/contractGenerators/rows")? {
            let input = string(member(row, "inputRef")?)?;
            let record =
                index(cell(row, "recordIndex")?.ok_or("present definition record index")?)?;
            if definitions.insert((input, record), row).is_some() {
                return Err("duplicate Generator definition occurrence".into());
            }
        }
        let mut titles = BTreeMap::new();
        for row in array(catalog, "/projections/missionTitles/rows")? {
            let occurrence = Occurrence::read(member(row, "contract")?)?;
            if titles.insert(occurrence, row).is_some() {
                return Err("duplicate mission title occurrence".into());
            }
        }
        let mut keys = BTreeMap::new();
        for row in array(catalog, "/texts/gameTexts")? {
            let id = string(cell(row, "id")?.ok_or("present game text ID")?)?;
            if keys.insert(id, row).is_some() {
                return Err("duplicate game text ID".into());
            }
        }
        Ok(Self {
            definitions,
            titles,
            keys,
        })
    }

    pub(crate) fn header(&self, row: &Value, field: &str) -> TestResult<&'a str> {
        let occurrence = Occurrence::read(member(row, "source")?)?;
        let definition = self
            .definitions
            .get(&occurrence.owner)
            .ok_or("owning Generator definition")?;
        string(cell(definition, field)?.ok_or("present Generator header field")?)
    }

    pub(crate) fn text(&self, row: &Value, field: &str) -> TestResult<Text<'a>> {
        let occurrence = Occurrence::read(member(row, "source")?)?;
        let title = self
            .titles
            .get(&occurrence)
            .ok_or("mission title for the contract occurrence")?;
        let text = member(title, field)?;
        let key = match cell(text, "key")? {
            Some(id) => {
                let entry = self
                    .keys
                    .get(string(id)?)
                    .ok_or("protected game text row for the key")?;
                Some(string(
                    cell(entry, "key")?.ok_or("present protected game text key")?,
                )?)
            }
            None => None,
        };
        let translations = member(text, "translations")?
            .as_object()
            .ok_or("translation cells")?;
        Ok(Text {
            key,
            en: translation(translations, "en")?,
            de: translation(translations, "de")?,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Text<'a> {
    pub(crate) key: Option<&'a str>,
    pub(crate) en: Option<&'a str>,
    pub(crate) de: Option<&'a str>,
}

fn translation<'a>(
    translations: &'a serde_json::Map<String, Value>,
    language: &str,
) -> TestResult<Option<&'a str>> {
    translations
        .get(language)
        .map(sourced)
        .transpose()?
        .flatten()
        .map(string)
        .transpose()
}

fn array<'a>(value: &'a Value, path: &str) -> TestResult<&'a [Value]> {
    value
        .pointer(path)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("expected array at {path}").into())
}

fn member<'a>(value: &'a Value, field: &str) -> TestResult<&'a Value> {
    value
        .get(field)
        .ok_or_else(|| format!("missing v3 field {field}").into())
}

fn string(value: &Value) -> TestResult<&str> {
    value.as_str().ok_or_else(|| "expected a string".into())
}

fn index(value: &Value) -> TestResult<u64> {
    value
        .as_u64()
        .ok_or_else(|| "expected a physical record index".into())
}

fn value_index(value: &Value) -> TestResult<&str> {
    string(member(value, "valueIndex")?)
}

pub(crate) fn cell<'a>(row: &'a Value, field: &str) -> TestResult<Option<&'a Value>> {
    sourced(member(row, field)?)
}

pub(crate) fn sourced(cell: &Value) -> TestResult<Option<&Value>> {
    let value = member(cell, "value")?;
    match (string(member(cell, "state")?)?, value.is_null()) {
        ("present", false) => Ok(Some(value)),
        ("absent" | "null" | "unreadable" | "unsupported" | "invalid", true) => Ok(None),
        _ => Err("inconsistent v3 sourced cell state and value".into()),
    }
}

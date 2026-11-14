// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::Language;
use serde_json::Value;

use crate::text_forms_fixture::{self as fixture, TestResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Role {
    Payload,
    Selector,
    Conversion,
    Attempted,
    Reference,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Contribution {
    Key {
        record: u64,
        field: String,
        role: Role,
    },
    Lookup {
        record: u64,
        field: String,
        role: Role,
    },
}

#[derive(Clone, Copy)]
pub(crate) enum Lookup {
    Placeholder,
    Missing,
    Source,
    DecodedSource,
    Display,
}

pub(crate) fn expected(owners: &[(u64, &str)], lookup: Lookup) -> BTreeSet<Contribution> {
    let mut result = BTreeSet::new();
    for &(record, field) in owners {
        result.insert(Contribution::Key {
            record,
            field: field.to_owned(),
            role: Role::Selector,
        });
        for &role in roles(lookup) {
            result.insert(Contribution::Lookup {
                record,
                field: field.to_owned(),
                role,
            });
        }
    }
    result
}

const fn roles(lookup: Lookup) -> &'static [Role] {
    match lookup {
        Lookup::Placeholder => &[],
        Lookup::Missing => &[Role::Selector],
        Lookup::Source => &[Role::Payload, Role::Selector],
        Lookup::DecodedSource | Lookup::Display => {
            &[Role::Payload, Role::Selector, Role::Conversion]
        }
    }
}

pub(crate) fn actual(
    catalog: &Value,
    carrier: &Value,
    language: Language,
) -> TestResult<BTreeSet<Contribution>> {
    let inventory = rows(catalog, "/provenance/uses")?;
    let sources = rows(catalog, "/provenance/sources")?;
    let references = fixture::at(carrier, "/sourceUsesRefs")?
        .as_array()
        .ok_or("immediate text input references")?;
    let mut seen = BTreeSet::new();
    let mut result = BTreeSet::new();
    for reference in references {
        let id = reference.as_str().ok_or("text input reference ID")?;
        if !seen.insert(id) {
            return Err("duplicate immediate text input reference".into());
        }
        let used = unique(inventory, reference)?;
        let contribution = contribution(sources, inventory, used, language)?;
        if !result.insert(contribution) {
            return Err("duplicate logical text contribution".into());
        }
    }
    Ok(result)
}

fn contribution(
    sources: &[Value],
    inventory: &[Value],
    used: &Value,
    language: Language,
) -> TestResult<Contribution> {
    Ok(match fixture::at(used, "/location/type")?.as_str() {
        Some("dataForge") => {
            binding(sources, used, "Data\\Game2.dcb")?;
            let (record, field) = body(used)?;
            Contribution::Key {
                record,
                field,
                role: role(used)?,
            }
        }
        Some("localizationLookup") => lookup(sources, inventory, used, language)?,
        _ => return Err("unexpected immediate text input coordinate".into()),
    })
}

fn lookup(
    sources: &[Value],
    inventory: &[Value],
    used: &Value,
    language: Language,
) -> TestResult<Contribution> {
    binding(sources, used, language.global_ini_path())?;
    if fixture::at(used, "/location/language")?.as_str() != Some(language.code()) {
        return Err("another language contributed to this text form".into());
    }
    let selector = unique(inventory, fixture::at(used, "/location/selector")?)?;
    binding(sources, selector, "Data\\Game2.dcb")?;
    if fixture::at(selector, "/location/type")?.as_str() != Some("dataForge")
        || role(selector)? != Role::Selector
    {
        return Err("lookup key is not an original DCB field selector".into());
    }
    let (record, field) = body(selector)?;
    Ok(Contribution::Lookup {
        record,
        field,
        role: role(used)?,
    })
}

fn binding(sources: &[Value], used: &Value, expected_path: &str) -> TestResult {
    if fixture::at(used, "/scope")?.as_str() != Some("payload") {
        return Err("a text contribution has the wrong input byte scope".into());
    }
    let source = unique(sources, fixture::at(used, "/source")?)?;
    if fixture::at(source, "/kind")?.as_str() != Some("archiveEntry")
        || fixture::at(source, "/path/value")?.as_str() != Some(expected_path)
    {
        return Err("a text contribution has the wrong original Archive input".into());
    }
    Ok(())
}

fn body(used: &Value) -> TestResult<(u64, String)> {
    Ok((
        fixture::at(used, "/location/recordIndex")?
            .as_u64()
            .ok_or("original key-owner Record index")?,
        fixture::at(used, "/location/field")?
            .as_str()
            .ok_or("original key-owner field")?
            .to_owned(),
    ))
}

fn role(used: &Value) -> TestResult<Role> {
    Ok(match fixture::at(used, "/role")?.as_str() {
        Some("payload") => Role::Payload,
        Some("selector") => Role::Selector,
        Some("conversion") => Role::Conversion,
        Some("attempted") => Role::Attempted,
        Some("reference") => Role::Reference,
        _ => return Err("unknown source-use role".into()),
    })
}

pub(crate) fn unique<'a>(rows: &'a [Value], id: &Value) -> TestResult<&'a Value> {
    let mut matches = rows.iter().filter(|row| row.get("id") == Some(id));
    let row = matches
        .next()
        .ok_or("unclosed original text input reference")?;
    if matches.next().is_some() {
        return Err("ambiguous original text input reference".into());
    }
    Ok(row)
}

fn rows<'a>(catalog: &'a Value, path: &str) -> TestResult<&'a [Value]> {
    fixture::at(catalog, path)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| "original source inventory".into())
}

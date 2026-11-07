// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_export::EXCLUDED_TOKENS;
use p4kx_extraction::{StructValue, Value};

use super::OracleResult;
use super::error::{Failure, SourceFault, SourceLevel};

type RowKey = (String, Option<String>);

pub(super) struct SourceSet<'t, 'f> {
    pub(super) level: SourceLevel,
    pub(super) properties: &'t [Value<'f>],
}

pub(super) struct Sources<'t, 'f> {
    pub(super) sets: HashMap<String, SourceSet<'t, 'f>>,
    pub(super) rows: HashMap<RowKey, Vec<String>>,
}

pub(super) fn read<'t, 'f>(root: &'t StructValue<'f>) -> OracleResult<Sources<'t, 'f>> {
    let mut out = Sources {
        sets: HashMap::new(),
        rows: HashMap::new(),
    };
    let handlers = items(root, "generators")?.iter().enumerate();
    for (index, handler) in handlers {
        let Some(handler) = structure(handler)? else {
            continue;
        };
        let path = format!("generators[{index}]");
        let handler_set = register(
            &mut out,
            parameter(handler, "contractParams")?,
            &format!("{path}.contractParams"),
            SourceLevel::Handler,
        )?;
        read_contracts(handler, &path, handler_set.as_ref(), &mut out)?;
    }
    Ok(out)
}

fn read_contracts<'t, 'f>(
    handler: &'t StructValue<'f>,
    path: &str,
    handler_set: Option<&String>,
    out: &mut Sources<'t, 'f>,
) -> OracleResult {
    for container in [
        "contracts",
        "introContracts",
        "legacyContracts",
        "PVPBountyContract",
    ] {
        let contracts = items(handler, container)?.iter().enumerate();
        for (index, contract) in contracts {
            let Some(contract) = structure(contract)? else {
                continue;
            };
            let path = format!("{path}.{container}[{index}]");
            let own = register(
                out,
                parameter(contract, "paramOverrides")?,
                &format!("{path}.paramOverrides"),
                SourceLevel::Contract,
            )?;
            let links: Vec<String> = handler_set
                .into_iter()
                .chain(own.as_ref())
                .cloned()
                .collect();
            let id = identity(contract);
            if let Some(id) = &id {
                out.rows
                    .entry((id.clone(), None))
                    .or_insert_with(|| links.clone());
            }
            read_subs(contract, &path, id.as_deref(), &links, out)?;
        }
    }
    Ok(())
}

fn read_subs<'t, 'f>(
    contract: &'t StructValue<'f>,
    path: &str,
    id: Option<&str>,
    links: &[String],
    out: &mut Sources<'t, 'f>,
) -> OracleResult {
    let subs = items(contract, "subContracts")?.iter().enumerate();
    for (index, sub) in subs {
        let Some(sub) = structure(sub)? else { continue };
        let path = format!("{path}.subContracts[{index}]");
        let (params, location) = match parameter(sub, "paramOverrides")? {
            Some(params) => (params, format!("{path}.paramOverrides")),
            None => (sub, path),
        };
        let own = register(out, Some(params), &location, SourceLevel::SubContract)?;
        let mut links = links.to_vec();
        links.extend(own);
        if let (Some(id), Some(sub_id)) = (id, identity(sub)) {
            out.rows
                .entry((id.to_owned(), Some(sub_id)))
                .or_insert(links);
        }
    }
    Ok(())
}

fn register<'t, 'f>(
    out: &mut Sources<'t, 'f>,
    params: Option<&'t StructValue<'f>>,
    path: &str,
    level: SourceLevel,
) -> OracleResult<Option<String>> {
    let Some(params) = params else {
        return Ok(None);
    };
    let Some(value) = params.field("propertyOverrides") else {
        return Ok(None);
    };
    if matches!(value, Value::Null) {
        return Ok(None);
    }
    let Value::Array(properties) = value else {
        return Err(Failure::SourceCompare(SourceFault::SourceArrayShape));
    };
    let location = format!("{path}.propertyOverrides");
    out.sets
        .insert(location.clone(), SourceSet { level, properties });
    Ok(Some(location))
}

fn identity(value: &StructValue<'_>) -> Option<String> {
    let name = match value.field("debugName") {
        Some(Value::String(name)) => *name,
        _ => "",
    };
    if name
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|word| {
            EXCLUDED_TOKENS
                .iter()
                .any(|token| word.eq_ignore_ascii_case(token))
        })
    {
        return None;
    }
    match value.field("id") {
        Some(Value::Guid(guid)) if *guid != p4kx_extraction::Guid::default() => {
            Some(guid.to_string())
        }
        _ => None,
    }
}

fn items<'t, 'f>(value: &'t StructValue<'f>, name: &str) -> OracleResult<&'t [Value<'f>]> {
    match value.field(name) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(values)) => Ok(values),
        _ => Err(Failure::SourceCompare(SourceFault::SourceArrayShape)),
    }
}

fn parameter<'t, 'f>(
    value: &'t StructValue<'f>,
    name: &str,
) -> OracleResult<Option<&'t StructValue<'f>>> {
    value
        .field(name)
        .map(structure)
        .transpose()
        .map(Option::flatten)
}

pub(super) const fn structure<'t, 'f>(
    value: &'t Value<'f>,
) -> OracleResult<Option<&'t StructValue<'f>>> {
    match value {
        Value::Null => Ok(None),
        Value::Struct(value) => Ok(Some(value)),
        _ => Err(Failure::SourceCompare(SourceFault::SourceStructShape)),
    }
}

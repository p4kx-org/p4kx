// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{DataForge, DataType, Property, Struct};
use serde::Serialize;

use super::error::{self, Error, Kind};

const ROOTS: [&str; 3] = ["ResourceType", "ResourceTypeGroup", "ResourceTypeProperty"];
const MAX_DEFINITIONS: usize = 1_000_000;
const MAX_SELECTED: usize = 10_000;
const MAX_FIELDS: usize = 1_000_000;
const MAX_CLOSURE_PASSES: usize = 64;
const MAX_NAME_BYTES: usize = 4_096;
const MAX_DECLARATION_TEXT: usize = 8 << 20;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Declarations {
    roots: Vec<&'static str>,
    missing_roots: Vec<&'static str>,
    types: Vec<TypeDeclaration>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TypeDeclaration {
    name: String,
    parent: Option<String>,
    fields: Vec<FieldDeclaration>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FieldDeclaration {
    name: String,
    data_type: String,
    array: bool,
    target_type: Option<String>,
}

fn name(text: &str) -> super::Result<&str> {
    if text.len() > MAX_NAME_BYTES || text.chars().any(char::is_control) {
        return Err(Error::new(Kind::Bounds, "schema name exceeds its declaration bound").into());
    }
    Ok(text)
}

fn definitions<'f>(
    forge: &'f DataForge<'f>,
    cancelled: &impl Fn() -> bool,
) -> super::Result<BTreeMap<&'f str, Struct<'f>>> {
    let mut definitions = BTreeMap::new();
    for structure in forge.structs() {
        error::check(cancelled)?;
        if definitions.len() >= MAX_DEFINITIONS {
            return Err(Error::new(Kind::Bounds, "schema definitions exceed their bound").into());
        }
        if definitions
            .insert(name(structure.name()?)?, structure)
            .is_some()
        {
            return Err(Error::new(Kind::Identity, "duplicate schema declaration name").into());
        }
    }
    Ok(definitions)
}

fn expand<'f>(
    definitions: &BTreeMap<&'f str, Struct<'f>>,
    wanted: &mut BTreeSet<&'f str>,
    polymorphic: &mut BTreeSet<&'f str>,
    cancelled: &impl Fn() -> bool,
) -> super::Result<()> {
    for (&type_name, structure) in definitions {
        error::check(cancelled)?;
        let parent = structure.parent().map(|parent| parent.name()).transpose()?;
        if parent.is_some_and(|parent| polymorphic.contains(parent)) {
            wanted.insert(type_name);
            polymorphic.insert(type_name);
        }
        if !wanted.contains(type_name) {
            continue;
        }
        if let Some(parent) = parent {
            wanted.insert(name(parent)?);
        }
        for property in structure.properties() {
            error::check(cancelled)?;
            let data_type = property.data_type()?;
            if !matches!(data_type, DataType::Class | DataType::StrongPointer) {
                continue;
            }
            let target = property.target_struct().ok_or_else(|| {
                Error::new(
                    Kind::Mapping,
                    "owned schema field has no declared target type",
                )
            })?;
            let target_name = name(target.name()?)?;
            wanted.insert(target_name);
            if data_type == DataType::StrongPointer {
                polymorphic.insert(target_name);
            }
        }
    }
    if wanted.len() > MAX_SELECTED {
        return Err(Error::new(Kind::Bounds, "owned declaration closure exceeds its bound").into());
    }
    Ok(())
}

fn field(property: Property<'_>) -> super::Result<FieldDeclaration> {
    let data_type = property.data_type()?;
    let target_type = match data_type {
        DataType::Enum => property
            .target_enum()
            .map(|target| target.name())
            .transpose()?,
        DataType::Class | DataType::StrongPointer | DataType::WeakPointer | DataType::Reference => {
            property
                .target_struct()
                .map(|target| target.name())
                .transpose()?
        }
        _ => None,
    };
    Ok(FieldDeclaration {
        name: name(property.name()?)?.to_owned(),
        data_type: format!("{data_type:?}"),
        array: property.is_array(),
        target_type: target_type.map(name).transpose()?.map(str::to_owned),
    })
}

fn declaration(
    structure: Struct<'_>,
    field_count: &mut usize,
    text_bytes: &mut usize,
    cancelled: &impl Fn() -> bool,
) -> super::Result<TypeDeclaration> {
    let parent = structure.parent().map(|parent| parent.name()).transpose()?;
    let mut result = TypeDeclaration {
        name: name(structure.name()?)?.to_owned(),
        parent: parent.map(name).transpose()?.map(str::to_owned),
        fields: Vec::new(),
    };
    *text_bytes += result.name.len() + result.parent.as_ref().map_or(0, String::len);
    if *text_bytes > MAX_DECLARATION_TEXT {
        return Err(Error::new(Kind::Bounds, "schema declaration text exceeds its bound").into());
    }
    let mut names = BTreeSet::new();
    for property in structure.properties() {
        error::check(cancelled)?;
        let field = field(property)?;
        if !names.insert(field.name.clone()) {
            return Err(Error::new(Kind::Identity, "duplicate declared field name").into());
        }
        *field_count += 1;
        *text_bytes += field.name.len()
            + field.data_type.len()
            + field.target_type.as_ref().map_or(0, String::len);
        if *field_count > MAX_FIELDS || *text_bytes > MAX_DECLARATION_TEXT {
            return Err(Error::new(Kind::Bounds, "schema declarations exceed their bound").into());
        }
        result.fields.push(field);
    }
    Ok(result)
}

pub(crate) fn collect(
    forge: &DataForge<'_>,
    cancelled: &impl Fn() -> bool,
) -> super::Result<Declarations> {
    let definitions = definitions(forge, cancelled)?;
    let mut wanted: BTreeSet<_> = ROOTS
        .into_iter()
        .filter(|root| definitions.contains_key(root))
        .collect();
    let missing_roots = ROOTS
        .into_iter()
        .filter(|root| !definitions.contains_key(root))
        .collect();
    if wanted.is_empty() {
        return Err(Error::new(
            Kind::Mapping,
            "no requested resource declaration root exists",
        )
        .into());
    }
    let mut polymorphic = wanted.clone();
    let mut complete = false;
    for _ in 0..MAX_CLOSURE_PASSES {
        let before = wanted.len() + polymorphic.len();
        expand(&definitions, &mut wanted, &mut polymorphic, cancelled)?;
        if before == wanted.len() + polymorphic.len() {
            complete = true;
            break;
        }
    }
    if !complete {
        return Err(Error::new(Kind::Bounds, "owned declaration closure did not converge").into());
    }
    let mut types = Vec::new();
    let mut fields = 0;
    let mut text_bytes = 0;
    for selected in wanted {
        error::check(cancelled)?;
        let structure = definitions
            .get(selected)
            .ok_or_else(|| Error::new(Kind::Mapping, "selected schema declaration disappeared"))?;
        types.push(declaration(
            *structure,
            &mut fields,
            &mut text_bytes,
            cancelled,
        )?);
    }
    Ok(Declarations {
        roots: ROOTS.to_vec(),
        missing_roots,
        types,
    })
}

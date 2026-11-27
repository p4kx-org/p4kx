// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{DataForge, DataType};

use super::error::{self, Error, Kind};
use super::inventory::FieldKey;

#[derive(Debug, Clone)]
pub(crate) struct Declaration {
    pub(crate) target: Option<String>,
    pub(crate) array: bool,
}

pub(crate) struct Schema {
    pub(crate) declarations: BTreeMap<FieldKey, Declaration>,
    pub(crate) owners: BTreeMap<String, BTreeMap<String, FieldKey>>,
}

fn definitions<'f>(
    forge: &'f DataForge<'f>,
    cancelled: &impl Fn() -> bool,
) -> super::Result<BTreeMap<String, p4kx_extraction::Struct<'f>>> {
    let mut definitions = BTreeMap::new();
    for structure in forge.structs() {
        error::check(cancelled)?;
        if definitions.len() >= 1_000_000 {
            return Err(Error::new(Kind::Bounds, "schema definitions exceed their bound").into());
        }
        if definitions
            .insert(structure.name()?.to_owned(), structure)
            .is_some()
        {
            return Err(Error::new(Kind::Identity, "duplicate source schema owner name").into());
        }
    }
    Ok(definitions)
}

impl Schema {
    pub(crate) fn read(
        forge: &DataForge<'_>,
        roots: &[String],
        cancelled: &impl Fn() -> bool,
    ) -> super::Result<Self> {
        let definitions = definitions(forge, cancelled)?;
        let mut wanted: BTreeSet<_> = roots.iter().cloned().collect();
        let mut polymorphic = wanted.clone();
        loop {
            error::check(cancelled)?;
            let before = wanted.len() + polymorphic.len();
            for (name, structure) in &definitions {
                let parent = structure.parent().map(|parent| parent.name()).transpose()?;
                if parent.is_some_and(|parent| polymorphic.contains(parent)) {
                    wanted.insert(name.clone());
                    polymorphic.insert(name.clone());
                }
                if !wanted.contains(name) {
                    continue;
                }
                if let Some(parent) = parent {
                    wanted.insert(parent.to_owned());
                }
                for property in structure.properties() {
                    let storage = property.data_type()?;
                    if matches!(storage, DataType::Class | DataType::StrongPointer)
                        && let Some(target) = property.target_struct()
                    {
                        let target = target.name()?;
                        wanted.insert(target.to_owned());
                        if storage == DataType::StrongPointer {
                            polymorphic.insert(target.to_owned());
                        }
                    }
                }
            }
            if wanted.len() + polymorphic.len() == before {
                break;
            }
            if wanted.len() > 10_000 {
                return Err(Error::new(Kind::Bounds, "schema closure exceeds its bound").into());
            }
        }
        let mut schema = Self {
            declarations: BTreeMap::new(),
            owners: BTreeMap::new(),
        };
        for name in wanted {
            error::check(cancelled)?;
            let Some(structure) = definitions.get(&name) else {
                continue;
            };
            schema.add(*structure, &name)?;
        }
        Ok(schema)
    }

    fn add(&mut self, structure: p4kx_extraction::Struct<'_>, name: &str) -> super::Result<()> {
        let fields = self.owners.entry(name.to_owned()).or_default();
        let mut current = Some(structure);
        let mut depth = 0;
        while let Some(owner) = current {
            depth += 1;
            if depth > 64 {
                return Err(
                    Error::new(Kind::Bounds, "schema inheritance exceeds its bound").into(),
                );
            }
            for property in owner.properties() {
                let mut key = FieldKey::new(owner.name()?, property.name()?);
                let declaration = Declaration {
                    target: property
                        .target_struct()
                        .map(|target| target.name().map(str::to_owned))
                        .transpose()?,
                    array: property.is_array(),
                };
                if fields
                    .insert(property.name()?.to_owned(), key.clone())
                    .is_some()
                {
                    return Err(Error::new(
                        Kind::Identity,
                        "duplicate or shadowed source field name",
                    )
                    .into());
                }
                self.declarations.insert(key.clone(), declaration.clone());
                if property.is_array()
                    && !matches!(
                        property.data_type()?,
                        DataType::Class
                            | DataType::StrongPointer
                            | DataType::WeakPointer
                            | DataType::Reference
                    )
                {
                    key.array_element = true;
                    self.declarations.insert(key, declaration);
                }
                if self.declarations.len() > 1_000_000 {
                    return Err(Error::new(Kind::Bounds, "schema fields exceed their bound").into());
                }
            }
            current = owner.parent();
        }
        Ok(())
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_dataforge::{DataForge, Guid, Value};
use p4kx_extraction::SourceLocation;

use super::{HeadItems, HeadMaterial, Wearable};
use crate::LoadoutResolver;
use crate::read::{as_struct, indexed_struct_items, text};
use crate::spec::PortSpec;

const GENERATION_PARAMS: &str = "SCharacterGenerationParams";

/// Supplies the items of a Character's head from the Records of a DataForge.
///
/// The Entities are those its item GUIDs name; the material libraries are those its material GUIDs
/// stand for, which the `materialLookupTable` of the Character generation parameters lists.
pub struct ForgeItems<'r, 'f> {
    resolver: &'r mut LoadoutResolver<'f>,
    materials: HashMap<Guid, MaterialTarget>,
}

pub(crate) struct MaterialTarget {
    pub(crate) path: String,
    pub(crate) record: u32,
    pub(crate) index: usize,
}

impl std::fmt::Debug for ForgeItems<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ForgeItems")
            .field("materials", &self.materials.len())
            .finish_non_exhaustive()
    }
}

impl<'r, 'f> ForgeItems<'r, 'f> {
    /// Reads the material lookup table of the forge of `resolver`, and supplies Entities through
    /// it.
    #[must_use]
    pub fn new(resolver: &'r mut LoadoutResolver<'f>) -> Self {
        let materials = lookup(resolver.entities().forge());
        Self {
            resolver,
            materials,
        }
    }

    /// Returns the names of the item ports the Entity `item` defines, empty when it has no Record.
    pub fn port_names(&mut self, item: Guid) -> Vec<String> {
        let mut names = Vec::new();
        self.visit_port_names(item, |_, port| names.push(port.name.clone()));
        names
    }

    pub(crate) fn visit_port_names(
        &mut self,
        item: Guid,
        mut visit: impl FnMut(Option<u32>, &PortSpec),
    ) {
        let spec = self.resolver.spec(item);
        if let Ok(spec) = spec.as_ref() {
            for port in &spec.ports {
                visit(spec.record_index, port);
            }
        }
    }

    pub(crate) fn material_target(&self, material: Guid) -> Option<&MaterialTarget> {
        self.materials.get(&material)
    }

    /// Returns how many material libraries the lookup table lists.
    #[must_use]
    pub fn material_count(&self) -> usize {
        self.materials.len()
    }
}

fn lookup(forge: &DataForge<'_>) -> HashMap<Guid, MaterialTarget> {
    let mut materials = HashMap::new();
    for record in forge.records() {
        if !record
            .type_name()
            .is_ok_and(|name| name == GENERATION_PARAMS)
        {
            continue;
        }
        let Ok(tree) = record.tree() else { continue };
        let Some(table) = tree.field("materialLookupTable").and_then(as_struct) else {
            continue;
        };
        for (index, entry) in indexed_struct_items(table, "materials") {
            let path = text(entry, "filePath");
            if let Some(Value::Guid(guid)) = entry.field("guid")
                && !path.is_empty()
            {
                materials.entry(*guid).or_insert_with(|| MaterialTarget {
                    path: path.to_owned(),
                    record: record.index(),
                    index,
                });
            }
        }
    }
    materials
}

impl HeadItems for ForgeItems<'_, '_> {
    fn wearable(&mut self, item: Guid) -> Result<Wearable, String> {
        let spec = self.resolver.spec(item);
        spec.as_ref()
            .as_ref()
            .map(Wearable::from_spec)
            .map_err(ToString::to_string)
    }

    fn material_path(&self, material: Guid) -> Option<String> {
        self.material_target(material)
            .map(|target| target.path.clone())
    }

    fn material_observed(&self, material: Guid) -> Option<HeadMaterial> {
        let target = self.material_target(material)?;
        let source = SourceLocation::DataForge {
            record_index: target.record,
            field: format!("materialLookupTable.materials[{}].filePath", target.index),
        };
        Some(HeadMaterial::new(target.path.clone(), Some(source)))
    }
}

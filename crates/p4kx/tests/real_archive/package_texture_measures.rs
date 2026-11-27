// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_blender::TextureRendition;
use serde::Deserialize as _;
use serde_json::Value;

use super::array;
use crate::package_fault::{self, Check, Fault};
use crate::package_types::Built;

pub(super) fn verify(built: &Built) -> package_fault::Result<usize> {
    let mut ids = HashMap::new();
    let mut files = HashSet::new();
    let mut sources = HashMap::<_, HashSet<_>>::new();
    for texture in array(&built.manifest, "textures")? {
        let id = string(texture, "id")?;
        let file = string(texture, "file")?;
        let source = string(texture, "source")?.to_ascii_lowercase();
        let rendition = TextureRendition::deserialize(
            texture
                .get("source_rendition")
                .ok_or(Check::ManifestField)?,
        )
        .map_err(|error| Fault::at(Check::ManifestField, error))?;
        package_fault::require(
            files.insert(file.to_ascii_lowercase()),
            Check::TextureDuplicateFile,
        )?;
        package_fault::require(
            ids.insert(id.to_ascii_lowercase(), id).is_none(),
            Check::TextureDuplicateId,
        )?;
        package_fault::require(
            sources.entry(source).or_default().insert(rendition),
            Check::TextureDuplicateRendition,
        )?;
        package_fault::require(built.out.join(file).is_file(), Check::TextureFile)?;
    }
    references(&built.manifest, &ids)?;
    let spent = array(&built.manifest, "issues")?.iter().any(|issue| {
        issue
            .get("reason")
            .and_then(Value::as_str)
            .is_some_and(|reason| reason.contains("budget"))
    });
    package_fault::require(!spent, Check::LayerBudget)?;
    Ok(sources.len())
}

fn references(manifest: &Value, ids: &HashMap<String, &str>) -> package_fault::Result<()> {
    for material in array(manifest, "materials")? {
        for binding in array(material, "textures")? {
            reference(binding.get("texture"), ids)?;
        }
    }
    for mesh in array(manifest, "meshes")? {
        for material in array(mesh, "materials")? {
            let bindings = material
                .get("textures")
                .and_then(Value::as_object)
                .ok_or(Check::ManifestField)?;
            for name in ["diffuse", "normal", "specular"] {
                reference(bindings.get(name), ids)?;
            }
        }
    }
    for texture in array(manifest, "textures")? {
        if let Some(pair) = texture.get("ddna_pair") {
            reference(
                Some(pair.get("smoothness").ok_or(Check::TextureReference)?),
                ids,
            )?;
        }
    }
    Ok(())
}

fn reference(wire: Option<&Value>, ids: &HashMap<String, &str>) -> package_fault::Result<()> {
    if let Some(value) = wire {
        let id = value.as_str().ok_or(Check::TextureReference)?;
        package_fault::require(
            ids.get(&id.to_ascii_lowercase()) == Some(&id),
            Check::TextureReference,
        )?;
    }
    Ok(())
}

fn string<'a>(row: &'a Value, field: &str) -> package_fault::Result<&'a str> {
    Ok(row
        .get(field)
        .and_then(Value::as_str)
        .ok_or(Check::ManifestField)?)
}

#[cfg(test)]
#[path = "package_texture_measure_tests.rs"]
mod tests;

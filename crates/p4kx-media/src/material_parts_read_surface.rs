// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::MaterialParameterInfo;
use serde_json::{Value, json};

use super::{Reader, invalid};
use crate::MediaError;
use crate::material_parts::{MaterialAttributeGroup as Group, MaterialPart, MaterialTree};

impl Reader<'_> {
    pub(super) fn surface(&mut self, material: usize, complete: bool) -> Result<Value, MediaError> {
        let MaterialPart::Surface {
            index,
            name,
            interpreted_surface,
            authored,
        } = self.part()?
        else {
            return Err(invalid());
        };
        if index != material || complete && authored.truncated != 0 {
            return Err(invalid());
        }
        let other = self.attributes(material, Group::Other, authored.other_attributes)?;
        let public = self.attributes(material, Group::PublicParam, authored.public_params)?;
        let mut layers = Vec::new();
        for position in 0..authored.layers {
            layers.push(self.layer(material, position)?);
        }
        let mut elements = Vec::new();
        for position in 0..authored.elements {
            elements.push(self.tree(MaterialTree::Material { material, position })?);
        }
        let mut truncated = Vec::new();
        for position in 0..authored.truncated {
            match self.part()? {
                MaterialPart::Truncation {
                    material: owner,
                    position: slot,
                    value,
                } if (owner, slot) == (material, position) => truncated.push(value),
                _ => return Err(invalid()),
            }
        }
        let mut textures = Vec::new();
        for position in 0..authored.textures {
            textures.push(self.texture(material, position)?);
        }
        let interpreted_surface: Value =
            serde_json::from_str(&serde_json::to_string(&interpreted_surface)?)?;
        Ok(json!({
            "index": index,
            "name": name,
            "interpreted_surface": interpreted_surface,
            "authored": {
                "other_attributes": other, "public_params": public, "layers": layers,
                "elements": elements, "truncated": truncated, "textures": textures,
            },
        }))
    }

    fn attributes(
        &mut self,
        material: usize,
        group: Group,
        count: usize,
    ) -> Result<Vec<MaterialParameterInfo<'static>>, MediaError> {
        let mut attributes = Vec::new();
        for position in 0..count {
            match self.part()? {
                MaterialPart::Attribute {
                    material: owner,
                    group: actual,
                    position: slot,
                    value,
                } if (owner, actual, slot) == (material, group, position) => attributes.push(value),
                _ => return Err(invalid()),
            }
        }
        Ok(attributes)
    }

    fn layer(&mut self, material: usize, position: usize) -> Result<Value, MediaError> {
        let MaterialPart::Layer {
            material: owner,
            position: slot,
            attributes,
        } = self.part()?
        else {
            return Err(invalid());
        };
        if (owner, slot) != (material, position) {
            return Err(invalid());
        }
        let attributes = self.attributes(material, Group::Layer { position }, attributes)?;
        Ok(json!({ "attributes": attributes }))
    }

    fn texture(&mut self, material: usize, position: usize) -> Result<Value, MediaError> {
        let MaterialPart::Texture {
            material: owner,
            position: slot,
            map,
            file,
            attributes,
            children,
        } = self.part()?
        else {
            return Err(invalid());
        };
        if (owner, slot) != (material, position) {
            return Err(invalid());
        }
        let attributes = self.attributes(material, Group::Texture { position }, attributes)?;
        let mut elements = Vec::new();
        for child in 0..children {
            elements.push(self.tree(MaterialTree::Texture {
                material,
                texture: position,
                position: child,
            })?);
        }
        Ok(json!({ "map": map, "file": file, "attributes": attributes, "children": elements }))
    }
}

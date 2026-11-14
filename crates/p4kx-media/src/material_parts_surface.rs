// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::model::{Attributes, Material, MaterialReadField as Field, TextureSlot};
use p4kx_extraction::{InputScope, MaterialParameterInfo, SourceUseRole as Role};

use super::{Output, capture, child, payload};
use crate::material_parts::{
    MaterialAttributeGroup as Group, MaterialAuthoredCounts, MaterialPart, MaterialTree,
};
use crate::material_surface::MaterialSurface;
use crate::origin_capture::OriginSite;
use crate::origin_reference::independent;
use crate::{MediaError, MediaWriter};

impl Output<'_, '_> {
    pub(super) fn material(
        &mut self,
        index: usize,
        material: &Material,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let position = self.reads.positions(index, Field::Row).map(<[u64]>::to_vec);
        self.surface(index, material, writer)?;
        self.attributes(index, Group::Other, &material.other_attributes, writer)?;
        self.attributes(index, Group::PublicParam, &material.public_params, writer)?;
        for (layer, value) in material.layers.iter().enumerate() {
            let mut origins = self.part_origins(writer)?;
            self.observed(
                &mut origins,
                &payload()?,
                index,
                &[("attributes", Field::Layer { index: layer })],
                writer,
            )?;
            self.emit(
                &MaterialPart::Layer {
                    material: index,
                    position: layer,
                    attributes: value.attributes.len(),
                },
                origins,
                writer,
            )?;
            self.attributes(
                index,
                Group::Layer { position: layer },
                &value.attributes,
                writer,
            )?;
        }
        for (block, element) in material.elements.iter().enumerate() {
            let actual = original_child(position.as_deref(), element.authored.ordinal)?;
            self.tree(
                MaterialTree::Material {
                    material: index,
                    position: block,
                },
                element,
                actual.as_deref(),
                writer,
            )?;
        }
        for (position, value) in material.truncated.iter().enumerate() {
            self.source_part(
                &MaterialPart::Truncation {
                    material: index,
                    position,
                    value: Cow::Borrowed(value),
                },
                "value",
                None,
                Role::Conversion,
                writer,
            )?;
        }
        for (position, texture) in material.textures.iter().enumerate() {
            self.texture(index, position, texture, writer)?;
        }
        Ok(())
    }

    fn surface(
        &mut self,
        index: usize,
        material: &Material,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut origins = self.part_origins(writer)?;
        let path = payload()?;
        self.observed(
            &mut origins,
            &path,
            index,
            &[("name", Field::Name), ("index", Field::Row)],
            writer,
        )?;
        let surface = child(&path, "interpreted_surface")?;
        self.observed(
            &mut origins,
            &surface,
            index,
            &[
                ("flags", Field::Flags),
                ("shader", Field::Shader),
                ("surface_type", Field::SurfaceType),
                ("string_gen_mask", Field::StringGenMask),
                ("diffuse", Field::Diffuse),
                ("specular", Field::Specular),
                ("emissive", Field::Emissive),
                ("shininess", Field::Shininess),
                ("opacity", Field::Opacity),
                ("alpha_test", Field::AlphaTest),
                ("glow", Field::Glow),
                ("palette", Field::Palette),
            ],
            writer,
        )?;
        for field in ["basis", "tinted", "decal_colours", "diffuse_map_is_grey"] {
            independent(&mut origins, &child(&surface, field)?, writer)?;
        }
        self.authored_counts(&mut origins, &path, index, writer)?;
        self.emit(
            &MaterialPart::Surface {
                index,
                name: Cow::Borrowed(&material.name),
                interpreted_surface: Box::new(MaterialSurface::of(material)),
                authored: MaterialAuthoredCounts::of(material),
            },
            origins,
            writer,
        )
    }

    fn authored_counts(
        &mut self,
        origins: &mut crate::origin_prepared::PreparedRecordOrigins,
        path: &p4kx_extraction::FieldPath,
        index: usize,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let authored = child(path, "authored")?;
        self.observed(
            origins,
            &authored,
            index,
            &[
                ("public_params", Field::PublicParams),
                ("layers", Field::Layers),
                ("textures", Field::Textures),
            ],
            writer,
        )?;
        let summary = capture(
            &self.input,
            InputScope::Payload,
            None,
            Role::Conversion,
            writer,
        )?;
        for field in ["other_attributes", "elements", "truncated"] {
            origins.insert_subtree(&child(&authored, field)?, summary.clone(), writer)?;
        }
        Ok(())
    }

    fn attributes(
        &mut self,
        material: usize,
        group: Group,
        attributes: &Attributes,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        for (position, (name, value)) in attributes.iter().enumerate() {
            let mut origins = self.part_origins(writer)?;
            let origin = match group {
                Group::PublicParam => self.reads.take(
                    material,
                    Field::PublicParam { index: position },
                    &self.input,
                    writer,
                )?,
                Group::Layer { position: index } => self.reads.take(
                    material,
                    Field::LayerAttribute {
                        index,
                        attribute: position,
                    },
                    &self.input,
                    writer,
                )?,
                Group::Other => self.reads.take(
                    material,
                    Field::OtherAttribute { index: position },
                    &self.input,
                    writer,
                )?,
                Group::Texture { position: index } => self.reads.take(
                    material,
                    Field::TextureAttribute {
                        index,
                        attribute: position,
                    },
                    &self.input,
                    writer,
                )?,
            };
            origins.insert_subtree(&child(&payload()?, "value")?, origin, writer)?;
            self.emit(
                &MaterialPart::Attribute {
                    material,
                    group,
                    position,
                    value: MaterialParameterInfo::from((name.as_str(), value.as_str())),
                },
                origins,
                writer,
            )?;
        }
        Ok(())
    }

    fn texture(
        &mut self,
        material: usize,
        position: usize,
        texture: &TextureSlot,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let field = Field::Texture { index: position };
        let indices = self.reads.positions(material, field).map(<[u64]>::to_vec);
        let mut origins = self.part_origins(writer)?;
        let path = payload()?;
        self.observed(
            &mut origins,
            &path,
            material,
            &[
                ("map", Field::TextureMap { index: position }),
                ("file", Field::TextureFile { index: position }),
            ],
            writer,
        )?;
        let site = indices
            .as_ref()
            .map(|indices| OriginSite::new("materialXmlElement", indices, "retainedTextureFields"));
        let selection = capture(
            &self.input,
            InputScope::Payload,
            site,
            Role::Conversion,
            writer,
        )?;
        for count in ["attributes", "children"] {
            origins.insert_subtree(&child(&path, count)?, selection.clone(), writer)?;
        }
        self.emit(
            &MaterialPart::Texture {
                material,
                position,
                map: Cow::Borrowed(&texture.map),
                file: Cow::Borrowed(&texture.file),
                attributes: texture.attributes.len(),
                children: texture.children.len(),
            },
            origins,
            writer,
        )?;
        self.attributes(
            material,
            Group::Texture { position },
            &texture.attributes,
            writer,
        )?;
        for (child, element) in texture.children.iter().enumerate() {
            let actual = original_child(indices.as_deref(), element.authored.ordinal)?;
            self.tree(
                MaterialTree::Texture {
                    material,
                    texture: position,
                    position: child,
                },
                element,
                actual.as_deref(),
                writer,
            )?;
        }
        Ok(())
    }
}

fn original_child(
    parent: Option<&[u64]>,
    child: Option<usize>,
) -> Result<Option<Vec<u64>>, MediaError> {
    parent
        .zip(child)
        .map(|(parent, child)| {
            let mut path = parent.to_vec();
            path.push(crate::shader_values::count(child)?);
            Ok(path)
        })
        .transpose()
}

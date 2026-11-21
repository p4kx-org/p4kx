// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_gltf::{DrawnMaterial, texture_path};
use p4kx_model::Material;

use crate::TextureUse;
use crate::roles::{classify_in, has_smoothness};
use crate::texture_encoding::TextureMode;
use crate::textures::Wanted;

use super::{Collected, Refused};

impl Collected {
    pub(super) fn want_textures(
        &mut self,
        material: &Material,
        drawn: Option<&DrawnMaterial>,
        emission: Option<&crate::PackageMaterialEmission>,
    ) -> Vec<Option<usize>> {
        material
            .textures
            .iter()
            .map(|slot| {
                let Some(path) = texture_path(&slot.file) else {
                    if !slot.file.trim_start().starts_with('$') {
                        self.refused.push(Refused {
                            file: slot.file.clone(),
                            material: material.name.clone(),
                        });
                    }
                    return None;
                };
                if emission
                    .and_then(|source| source.preview.as_ref())
                    .is_some_and(|source| source.matches(&slot.map, &slot.file))
                {
                    return Some(self.want(&path, &slot.file, TextureMode::Colour, &material.name));
                }
                let role = classify_in(material, &slot.map, &slot.file);
                let mode = drawn
                    .and_then(|drawn| drawn_mode(drawn, &path, role))
                    .unwrap_or_else(|| TextureMode::of(role, has_smoothness(&slot.file)));
                Some(self.want(&path, &slot.file, mode, &material.name))
            })
            .collect()
    }

    pub(super) fn want_drawn(&mut self, drawn: &DrawnMaterial) {
        for (texture, role) in [
            (drawn.diffuse.as_ref(), TextureUse::Diffuse),
            (drawn.normal.as_ref(), TextureUse::Normal),
            (drawn.specular.as_ref(), TextureUse::Specular),
        ] {
            if let Some(file) = texture {
                self.want(
                    &file.path,
                    &file.file,
                    TextureMode::drawn(file, role),
                    &drawn.name,
                );
            }
        }
    }

    pub(super) fn want(
        &mut self,
        path: &str,
        file: &str,
        mode: TextureMode,
        material: &str,
    ) -> usize {
        let identity = (path.to_ascii_lowercase(), mode);
        if let Some(&position) = self.index_of.get(&identity) {
            return position;
        }
        let position = self.wanted.len();
        self.index_of.insert(identity, position);
        self.wanted.push(Wanted {
            path: path.to_owned(),
            file: file.to_owned(),
            material: material.to_owned(),
            mode,
        });
        position
    }
}

fn drawn_mode(drawn: &DrawnMaterial, path: &str, role: TextureUse) -> Option<TextureMode> {
    let role_for = |file: &p4kx_gltf::TextureFile, role| {
        file.path
            .eq_ignore_ascii_case(path)
            .then(|| TextureMode::drawn(file, role))
    };
    match role {
        TextureUse::Diffuse | TextureUse::Mask | TextureUse::Stencil => drawn
            .diffuse
            .as_ref()
            .and_then(|file| role_for(file, TextureUse::Diffuse)),
        TextureUse::Normal => drawn
            .normal
            .as_ref()
            .and_then(|file| role_for(file, TextureUse::Normal)),
        TextureUse::Specular => drawn
            .specular
            .as_ref()
            .and_then(|file| role_for(file, TextureUse::Specular)),
        _ => None,
    }
}

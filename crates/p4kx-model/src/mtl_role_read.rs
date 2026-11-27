// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::TextureRole;
use crate::mtl_texture::{self, DIFFUSE_SLOT};

#[derive(Clone, Copy)]
pub(crate) enum TextureInput {
    Map,
    File,
}

pub(crate) fn role<Interrupted>(
    slot: &str,
    file: &str,
    observer: &mut impl FnMut(TextureInput) -> Result<(), Interrupted>,
) -> Result<TextureRole, Interrupted> {
    observer(TextureInput::File)?;
    let role = match mtl_texture::texture_suffix_lowercase(file).as_str() {
        "ddna" | "ddn" => TextureRole::Normal,
        "spec" => TextureRole::Specular,
        "mask" | "opac" => TextureRole::Mask,
        "blend" => TextureRole::Blend,
        "detail" => TextureRole::Detail,
        "wear" => TextureRole::Wear,
        "ao" => TextureRole::Occlusion,
        "displ" | "disp" => TextureRole::Displacement,
        "gloss" => TextureRole::Gloss,
        "id" | "scalp" | "dir" | "dirm" | "hal" => TextureRole::Other,
        _ => {
            observer(TextureInput::Map)?;
            if slot.eq_ignore_ascii_case(DIFFUSE_SLOT) {
                TextureRole::Diffuse
            } else {
                TextureRole::Other
            }
        }
    };
    Ok(role)
}

pub(crate) fn diffuse<Interrupted>(
    slot: &str,
    file: &str,
    observer: &mut impl FnMut(TextureInput) -> Result<(), Interrupted>,
) -> Result<bool, Interrupted> {
    let file = file.trim();
    observer(TextureInput::File)?;
    Ok(!file.is_empty()
        && !file.starts_with('$')
        && role(slot, file, observer)? == TextureRole::Diffuse)
}

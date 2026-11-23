// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::glb::CheckedGlb;

/// An image of a GLB decoded from its PNG to 8-bit RGBA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
    /// Four bytes per pixel, row by row from the top.
    pub rgba: Vec<u8>,
}

impl DecodedImage {
    /// Returns the pixel at column `x`, row `y`.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        let at = (usize::try_from(y).ok()? * usize::try_from(self.width).ok()?
            + usize::try_from(x).ok()?)
            * 4;
        self.rgba.get(at..at + 4)?.try_into().ok()
    }
}

impl CheckedGlb {
    /// Returns the PNG bytes of `image`, which the binary chunk holds.
    ///
    /// # Errors
    ///
    /// Returns a description when the image is no buffer view of a PNG or exceeds the chunk.
    pub fn png(&self, image: &gltf::Image<'_>) -> Result<&[u8], String> {
        let gltf::image::Source::View { view, mime_type } = image.source() else {
            return Err(format!("image {} is no buffer view", image.index()));
        };
        if mime_type != "image/png" {
            return Err(format!("image {} has the type {mime_type}", image.index()));
        }
        let end = view.offset().checked_add(view.length());
        end.and_then(|end| self.blob.get(view.offset()..end))
            .ok_or_else(|| format!("image {} exceeds the binary chunk", image.index()))
    }

    /// Decodes the PNG of `image` to 8-bit RGBA.
    ///
    /// # Errors
    ///
    /// Returns a description when the PNG is invalid or not 8-bit RGBA.
    pub fn decode(&self, image: &gltf::Image<'_>) -> Result<DecodedImage, String> {
        let bytes = self.png(image)?;
        let describe = |error: png::DecodingError| format!("image {}: {error}", image.index());
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .map_err(describe)?;
        let mut rgba = vec![0; reader.output_buffer_size().unwrap_or_default()];
        let frame = reader.next_frame(&mut rgba).map_err(describe)?;
        if frame.color_type != png::ColorType::Rgba || frame.bit_depth != png::BitDepth::Eight {
            return Err(format!("image {} is no 8-bit RGBA", image.index()));
        }
        rgba.truncate(frame.buffer_size());
        Ok(DecodedImage {
            width: frame.width,
            height: frame.height,
            rgba,
        })
    }
}

pub(crate) fn check_textures(glb: &CheckedGlb) -> Result<(), String> {
    for image in glb.document.images() {
        let decoded = glb.decode(&image)?;
        if decoded.width == 0 || decoded.height == 0 {
            return Err(format!("image {} is empty", image.index()));
        }
    }
    let mut image_used = vec![false; glb.document.images().len()];
    for texture in glb.document.textures() {
        let used = image_used
            .get_mut(texture.source().index())
            .ok_or_else(|| format!("texture {} has no image", texture.index()))?;
        *used = true;
        if texture
            .sampler()
            .index()
            .is_some_and(|sampler| sampler >= glb.document.samplers().len())
        {
            return Err(format!("texture {} has no sampler", texture.index()));
        }
    }
    if let Some(orphan) = image_used.iter().position(|used| !used) {
        return Err(format!("image {orphan} is used by no texture"));
    }
    let mut texture_used = vec![false; glb.document.textures().len()];
    for material in glb.document.materials() {
        for index in material_texture_indices(&material).into_iter().flatten() {
            *texture_used.get_mut(index).ok_or_else(|| {
                format!(
                    "material {} has no texture {index}",
                    material.index().unwrap_or(0)
                )
            })? = true;
        }
    }
    if let Some(orphan) = texture_used.iter().position(|used| !used) {
        return Err(format!("texture {orphan} is used by no material"));
    }
    Ok(())
}

fn material_texture_indices(material: &gltf::Material<'_>) -> [Option<usize>; 7] {
    let pbr = material.pbr_metallic_roughness();
    let specular = material.specular();
    [
        pbr.base_color_texture().map(|info| info.texture().index()),
        pbr.metallic_roughness_texture()
            .map(|info| info.texture().index()),
        material.normal_texture().map(|info| info.texture().index()),
        material
            .occlusion_texture()
            .map(|info| info.texture().index()),
        material
            .emissive_texture()
            .map(|info| info.texture().index()),
        specular
            .as_ref()
            .and_then(gltf::material::Specular::specular_texture)
            .map(|info| info.texture().index()),
        specular
            .as_ref()
            .and_then(gltf::material::Specular::specular_color_texture)
            .map(|info| info.texture().index()),
    ]
}

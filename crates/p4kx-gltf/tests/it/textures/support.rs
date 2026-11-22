// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The textures held in memory, the cube model and the helpers that read a GLB back.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::{Mutex, PoisonError};

use p4kx_chunks::ChunkFile;
use p4kx_gltf::{
    GlbOptions, GlbSummary, LoadedTexture, MaterialMode, TextureLoadError, TextureLoader,
    write_glb_with,
};
use p4kx_model::{CompanionError, CompanionPath, Model};
use p4kx_testkit::{CheckedGlb, DecodedImage, TestModel, TestMtl, TestMtlMaterial, check_glb};
use p4kx_texture::RgbaImage;

pub(crate) type TestResult = Result<(), Box<dyn Error>>;

pub(crate) const SIDE: u32 = 2;

#[derive(Default)]
pub(crate) struct Stub {
    textures: HashMap<String, Result<LoadedTexture, TextureLoadError>>,
    calls: Mutex<Vec<(String, u32)>>,
    sized: bool,
    sizes: HashMap<String, (u32, u32)>,
    unknown_sizes: HashSet<String>,
}

impl Stub {
    pub(crate) fn with(mut self, path: &str, texture: &LoadedTexture) -> Self {
        self.textures
            .insert(path.to_ascii_lowercase(), Ok(texture.clone()));
        self
    }

    /// Makes the stub tell the sizes of its textures without decoding them.
    pub(crate) const fn sized(mut self) -> Self {
        self.sized = true;
        self
    }

    /// Makes the stub tell `size` for the texture at `path`, whatever it holds.
    pub(crate) fn sized_as(mut self, path: &str, size: (u32, u32)) -> Self {
        self.sizes.insert(path.to_ascii_lowercase(), size);
        self
    }

    pub(crate) fn unknown_size(mut self, path: &str) -> Self {
        self.unknown_sizes.insert(path.to_ascii_lowercase());
        self
    }

    pub(crate) fn failing(mut self, path: &str, error: TextureLoadError) -> Self {
        self.textures.insert(path.to_ascii_lowercase(), Err(error));
        self
    }

    pub(crate) fn calls(&self) -> Vec<(String, u32)> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl TextureLoader for Stub {
    fn dimensions(&self, path: &str, _mip: u32) -> Result<Option<(u32, u32)>, TextureLoadError> {
        let key = path.to_ascii_lowercase();
        if let Some(size) = self.sizes.get(&key) {
            return Ok(Some(*size));
        }
        if !self.sized || self.unknown_sizes.contains(&key) {
            return Ok(None);
        }
        match self.textures.get(&key) {
            Some(Ok(texture)) => Ok(Some((texture.image.width(), texture.image.height()))),
            Some(Err(error)) => Err(error.clone()),
            None => Err(TextureLoadError::NotFound),
        }
    }

    fn load(&self, path: &str, mip: u32) -> Result<LoadedTexture, TextureLoadError> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((path.to_owned(), mip));
        self.textures
            .get(&path.to_ascii_lowercase())
            .cloned()
            .unwrap_or(Err(TextureLoadError::NotFound))
    }
}

pub(crate) fn texture(rgba: [u8; 4], has_alpha: bool) -> Result<LoadedTexture, Box<dyn Error>> {
    let pixels = rgba.repeat((SIDE * SIDE) as usize);
    Ok(LoadedTexture::new(
        RgbaImage::new(SIDE, SIDE, pixels)?,
        has_alpha,
    ))
}

pub(crate) fn model_with(mtl: Vec<u8>) -> Result<Model, Box<dyn Error>> {
    let cube = TestModel::cube();
    let cgfm = cube.cgfm();
    let mut companions = move |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Sibling("cube.cgfm") => Some(cgfm.clone()),
            CompanionPath::Material("objects/test/cube") => Some(mtl.clone()),
            _ => None,
        })
    };
    Ok(Model::read(
        "cube.cgf",
        &ChunkFile::parse(cube.cgf())?,
        &mut companions,
    )?)
}

pub(crate) fn textured() -> GlbOptions {
    GlbOptions::default().with_materials(MaterialMode::Textures)
}

pub(crate) fn export(
    materials: &[TestMtlMaterial],
    stub: &Stub,
    options: &GlbOptions,
) -> Result<(CheckedGlb, GlbSummary), Box<dyn Error>> {
    let model = model_with(TestMtl::multi(materials).cryxml())?;
    let mut bytes = Vec::new();
    let summary = write_glb_with(&model, options, stub, &mut bytes)?;
    assert_eq!(summary.bytes, bytes.len() as u64, "summary length");
    Ok((check_glb(&bytes)?, summary))
}

pub(crate) fn plate(name: &str) -> TestMtlMaterial {
    TestMtlMaterial::new(name)
        .diffuse([1.0; 3])
        .specular([1.0; 3])
        .texture("TexSlot1", "Objects/Test/plate_diff.tif")
        .texture("TexSlot2", "Objects/Test/plate_ddna.tif")
        .texture("TexSlot4", "Objects/Test/plate_spec.tif")
}

pub(crate) fn plain(name: &str) -> TestMtlMaterial {
    TestMtlMaterial::new(name).diffuse([0.25, 0.5, 0.75])
}

pub(crate) fn material<'a>(
    glb: &'a CheckedGlb,
    name: &str,
) -> Result<p4kx_testkit::gltf::Material<'a>, Box<dyn Error>> {
    glb.document
        .materials()
        .find(|material| material.name() == Some(name))
        .ok_or_else(|| format!("no material {name}").into())
}

pub(crate) fn image_of(
    glb: &CheckedGlb,
    texture: Option<p4kx_testkit::gltf::texture::Texture<'_>>,
) -> Result<DecodedImage, Box<dyn Error>> {
    let texture = texture.ok_or("a texture")?;
    Ok(glb.decode(&texture.source())?)
}

pub(crate) fn near(pixel: Option<[u8; 4]>, wanted: [u8; 4]) -> bool {
    pixel.is_some_and(|pixel| {
        pixel
            .iter()
            .zip(wanted)
            .all(|(found, wanted)| found.abs_diff(wanted) <= 1)
    })
}

pub(crate) fn plate_stub(
    diffuse: [u8; 4],
    ddna: [u8; 4],
    spec: [u8; 4],
) -> Result<Stub, Box<dyn Error>> {
    Ok(Stub::default()
        .with("objects/test/plate_diff.dds", &texture(diffuse, false)?)
        .with("objects/test/plate_ddna.dds", &texture(ddna, true)?)
        .with("objects/test/plate_spec.dds", &texture(spec, false)?))
}

pub(crate) fn base_image(
    glb: &CheckedGlb,
    material: &p4kx_testkit::gltf::Material<'_>,
) -> Result<DecodedImage, Box<dyn Error>> {
    let pbr = material.pbr_metallic_roughness();
    image_of(glb, pbr.base_color_texture().map(|info| info.texture()))
}

pub(crate) fn packed_image(
    glb: &CheckedGlb,
    material: &p4kx_testkit::gltf::Material<'_>,
) -> Result<DecodedImage, Box<dyn Error>> {
    let pbr = material.pbr_metallic_roughness();
    image_of(
        glb,
        pbr.metallic_roughness_texture().map(|info| info.texture()),
    )
}

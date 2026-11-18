// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

use p4kx_extraction::{
    ArchiveTextures, ArchiveWriteProtection, CancelToken, ModelArchiveSource, PathIndex,
};
use p4kx_gltf::{
    GlbOptions, LoadedTexture, MaterialMode, Scene, SceneNode, ScenePart, TextureLoadError,
    TextureLoader, node_transform, write_scene_glb_with,
};
use p4kx_model::{Model, TintPalette, Transform};
use p4kx_testkit::check_glb;
use serde_json::{Value, json};

use super::Check;
use super::input::{Request, canonical};
use super::materials::Loaded;
use crate::items::{Drawn, Item};
use crate::manifest::{EntityRef, NodeKind};
use crate::write::{self, PackageInfo};
use crate::{Manifest, PackageWriteContext};

pub(super) struct ExportedSubset {
    pub(super) manifest: Manifest,
    pub(super) glb_projection: Value,
    pub(super) requests: Vec<(String, u32)>,
}

struct Textures<'i, 'a> {
    source: ArchiveTextures<'i, 'a>,
    admitted: BTreeSet<String>,
    calls: Mutex<Vec<(String, u32)>>,
}

impl TextureLoader for Textures<'_, '_> {
    fn load(&self, path: &str, mip: u32) -> Result<LoadedTexture, TextureLoadError> {
        self.admit(path, mip)?;
        self.source.load(path, mip)
    }
    fn dimensions(&self, path: &str, mip: u32) -> Result<Option<(u32, u32)>, TextureLoadError> {
        self.admit(path, mip)?;
        self.source.dimensions(path, mip)
    }
}

impl Textures<'_, '_> {
    fn admit(&self, path: &str, mip: u32) -> Result<(), TextureLoadError> {
        if !self.admitted.contains(&canonical(path)) {
            return Err(TextureLoadError::Unreadable(
                "outside the prepared source selection".to_owned(),
            ));
        }
        self.calls
            .lock()
            .map_err(|_| TextureLoadError::Unreadable("probe request lock".to_owned()))?
            .push((path.to_owned(), mip));
        Ok(())
    }
}

pub(super) struct View<'a> {
    pub(super) model: &'a Model,
    pub(super) sources: &'a [ModelArchiveSource],
    pub(super) loaded: &'a Loaded,
    pub(super) palette: &'a TintPalette,
}

pub(super) fn write(
    index: &PathIndex<'_>,
    request: &Request,
    out: &Path,
    view: &View<'_>,
) -> Check<ExportedSubset> {
    let textures = Textures {
        source: ArchiveTextures::new(index),
        admitted: request
            .textures
            .iter()
            .map(|path| canonical(path))
            .collect(),
        calls: Mutex::default(),
    };
    let options = GlbOptions::default()
        .with_texture_mip(request.mip)
        .with_materials(MaterialMode::Textures);
    let glb_projection = glb(&textures, &options, request, out, view)?;
    let manifest = package(index, &textures, &options, request, out, view)?;
    let requests = textures
        .calls
        .into_inner()
        .map_err(|_| "probe request lock")?;
    Ok(ExportedSubset {
        manifest,
        glb_projection,
        requests,
    })
}

fn glb(
    textures: &Textures<'_, '_>,
    options: &GlbOptions,
    request: &Request,
    out: &Path,
    view: &View<'_>,
) -> Check<Value> {
    let mut scene = Scene::new();
    scene.push(
        SceneNode::new("selected original subset", None, Transform::IDENTITY)
            .with_part(ScenePart::new(view.model, &view.loaded.status).with_palette(view.palette)),
    )?;
    let mut bytes = Vec::new();
    let summary = write_scene_glb_with(&scene, options, textures, &mut bytes)?;
    if summary.triangles != request.triangles
        || summary.primitives != 1
        || !summary.texture_fallbacks.is_empty()
        || summary.default_material_ids != 0
    {
        return Err("the ordinary textured GLB did not retain exactly the selected subset".into());
    }
    verify_uvs(&bytes, view.model, request)?;
    fs::write(out.join("textured.glb"), &bytes)?;
    roughness(&bytes)
}

fn package(
    index: &PathIndex<'_>,
    textures: &Textures<'_, '_>,
    options: &GlbOptions,
    request: &Request,
    out: &Path,
    view: &View<'_>,
) -> Check<Manifest> {
    let raw = ArchiveTextures::new(index);
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(index.archive().path(), cancel.flag())?;
    let context =
        PackageWriteContext::new(&protection, cancel.flag()).with_archive_texture_source(&raw);
    publish_package(textures, options, request, out, view, &context)?;
    let manifest: Manifest = serde_json::from_slice(&fs::read(out.join("package/scene.json"))?)?;
    verify_package(&manifest, request)?;
    let mesh = manifest.meshes.first().ok_or("one ordinary package mesh")?;
    verify_uvs(
        &fs::read(out.join("package").join(&mesh.file))?,
        view.model,
        request,
    )?;
    Ok(manifest)
}

fn publish_package(
    textures: &Textures<'_, '_>,
    options: &GlbOptions,
    request: &Request,
    out: &Path,
    view: &View<'_>,
    context: &PackageWriteContext<'_>,
) -> Check {
    let entity = EntityRef {
        class_name: "selected original subset".to_owned(),
        guid: String::new(),
        classification: None,
    };
    let item = item(view.model, view.sources, view.loaded, view.palette);
    preflight(&item, view.loaded, options, request, context)?;
    let scene = write::Scene {
        entity,
        items: vec![item],
        issues: &[],
        authored_soc: None,
        classification: None,
    };
    write::write_scene(
        &scene,
        view.loaded,
        textures,
        options,
        &PackageInfo::default(),
        &out.join("package"),
        context,
    )?;
    Ok(())
}

fn item<'a>(
    model: &'a Model,
    sources: &'a [ModelArchiveSource],
    loaded: &'a Loaded,
    palette: &'a TintPalette,
) -> Item<'a> {
    Item {
        kind: NodeKind::Object,
        name: "selected original subset",
        path: String::new(),
        parent: None,
        transform: node_transform(&Transform::IDENTITY),
        entity: None,
        geometry: Some(&model.name),
        drawn: Some(Drawn {
            model,
            model_sources: sources,
            materials: &loaded.status,
            material_entry: Some(&loaded.original),
            palette: Some(palette),
            palette_source: None,
            palette_parent: None,
            label: "selected original subset",
        }),
        light: None,
        light_placement: None,
    }
}

fn preflight(
    item: &Item<'_>,
    loaded: &Loaded,
    options: &GlbOptions,
    request: &Request,
    context: &PackageWriteContext<'_>,
) -> Check {
    let one = std::slice::from_ref(item);
    let plan = crate::plan::MeshPlan::of(one);
    let collected = crate::collect::Collected::of(one, &plan, options.lod, loaded, None, context)?;
    if !collected.refused.is_empty()
        || collected.wanted.iter().any(|row| {
            !request
                .textures
                .iter()
                .any(|path| canonical(path) == canonical(&row.path))
        })
    {
        return Err("ordinary package collection exceeds the prepared texture closure".into());
    }
    Ok(())
}

fn verify_package(manifest: &Manifest, request: &Request) -> Check {
    let [mesh] = manifest.meshes.as_slice() else {
        return Err("one selected package mesh".into());
    };
    if mesh.triangles != request.triangles
        || mesh.vertex_map.is_some()
        || !manifest.issues.is_empty()
        || manifest.materials.len() != 1 + request.layers.len()
        || manifest.palettes.len() != 1
    {
        return Err(
            "the ordinary package is incomplete or extends the selected material closure".into(),
        );
    }
    for texture in &manifest.textures {
        if texture
            .source_metadata
            .as_ref()
            .is_none_or(|source| source.mip_level != request.mip)
        {
            return Err("the ordinary package lacks actual selected mip input evidence".into());
        }
    }
    Ok(())
}

fn verify_uvs(bytes: &[u8], model: &Model, request: &Request) -> Check {
    let checked = check_glb(bytes)?;
    let original = &model.lods[0].meshes[request.mesh].uvs;
    let expected: Vec<_> = original
        .iter()
        .flatten()
        .copied()
        .map(f32::to_bits)
        .collect();
    let retained = checked
        .document
        .accessors()
        .filter(|accessor| {
            accessor.dimensions().multiplicity() == 2 && accessor.data_type().size() == 4
        })
        .any(|accessor| {
            checked
                .floats(&accessor)
                .into_iter()
                .map(f32::to_bits)
                .eq(expected.iter().copied())
        });
    if !retained {
        return Err("the producer changed or discarded original source UV bits".into());
    }
    Ok(())
}

fn roughness(bytes: &[u8]) -> Check<Value> {
    let checked = check_glb(bytes)?;
    let material = checked
        .document
        .materials()
        .next()
        .ok_or("one selected GLB material")?;
    let pbr = material.pbr_metallic_roughness();
    let factor = pbr.roughness_factor();
    let texture = pbr
        .metallic_roughness_texture()
        .ok_or("sampled selected roughness")?;
    let source = texture.texture().source();
    let image = checked.decode(&source)?;
    let mut samples = Vec::new();
    for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        let pixel = image.pixel(x, y).ok_or("bound packed output coordinate")?;
        samples.push(json!({"pixel":[x,y],"greenByte":pixel[1],
            "effectiveRoughness":f32::from(pixel[1])/255.0*factor}));
    }
    Ok(
        json!({"roughnessFactor":factor,"textureCoordinateSet":texture.tex_coord(),
        "outputSamples":samples,"linearGreenTimesFactor":true,"sourceUvMappingClaim":false}),
    )
}

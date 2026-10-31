// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The material descriptions of a package written from the synthetic ship `TEST_Layered`:
//! shaders, layers and their materials, palettes, textures by use, placeholders and raw
//! parameters.

use p4kx_blender::{
    AlphaMeaning, AlphaMode, ColourSpace, FORMAT_VERSION, IssueOrigin, Manifest,
    MaterialDescription, PackageInfo, PaletteEntry, TextureUse, manifest_schema, write_package,
};
use p4kx_extraction::Archive;
use p4kx_model::srgb8_to_linear;
use p4kx_testkit::{LAYERED_MISSING_TEXTURE, LAYERED_TINTS, layered_entries};

use crate::support::{TestResult, assemble, fixture, glb_options, never};

pub(crate) struct Written {
    pub(crate) dir: tempfile::TempDir,
    pub(crate) manifest: Manifest,
    raw: serde_json::Value,
}

impl Written {
    pub(crate) fn material(&self, name: &str) -> Result<&MaterialDescription, String> {
        self.manifest
            .materials
            .iter()
            .find(|material| material.name == name)
            .ok_or_else(|| format!("no material {name}"))
    }

    fn by_id(&self, id: &str) -> Result<&MaterialDescription, String> {
        self.manifest
            .materials
            .iter()
            .find(|material| material.id == id)
            .ok_or_else(|| format!("no material {id}"))
    }
}

pub(crate) fn write() -> Result<Written, Box<dyn std::error::Error>> {
    let fixture = fixture(layered_entries())?;
    let archive = Archive::open(&fixture.path)?;
    let assembly = assemble(&archive, "TEST_Layered")?;
    let models = p4kx_assembly::ArchiveModels::new(&archive);
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("ship");
    write_package(
        &assembly,
        &models,
        &models.textures(),
        &glb_options(0),
        &PackageInfo::default(),
        &out,
        &never,
    )?;
    let text = std::fs::read_to_string(out.join("scene.json"))?;
    Ok(Written {
        manifest: serde_json::from_str(&text)?,
        raw: serde_json::from_str(&text)?,
        dir,
    })
}

fn parameter<'a>(material: &'a MaterialDescription, name: &str) -> Option<&'a str> {
    material
        .parameters
        .iter()
        .find(|parameter| parameter.name == name)
        .map(|parameter| parameter.value.as_str())
}

#[test]
fn the_manifest_is_version_1_2_and_matches_its_schema() -> TestResult {
    let written = write()?;
    let validator = jsonschema::draft202012::new(&manifest_schema())?;

    assert_eq!(written.manifest.version, FORMAT_VERSION);
    let violations: Vec<String> = validator
        .iter_errors(&written.raw)
        .take(5)
        .map(|error| format!("{} at {}", error, error.instance_path()))
        .collect();
    assert!(violations.is_empty(), "{violations:?}");
    Ok(())
}

#[test]
fn a_version_1_0_manifest_without_descriptions_still_reads() -> TestResult {
    let written = write()?;
    let mut raw = written.raw;
    if let Some(object) = raw.as_object_mut() {
        object.remove("materials");
        object.remove("palettes");
    }
    let validator = jsonschema::draft202012::new(&manifest_schema())?;

    assert!(validator.is_valid(&raw));
    let manifest: Manifest = serde_json::from_value(raw)?;
    assert!(manifest.materials.is_empty() && manifest.palettes.is_empty());
    Ok(())
}

#[test]
fn every_drawn_material_of_a_mesh_names_its_description() -> TestResult {
    let written = write()?;
    let mesh = written.manifest.meshes.first().ok_or("mesh")?;

    let described: Vec<&str> = mesh
        .materials
        .iter()
        .map(|material| {
            let id = material.material.as_deref().unwrap_or_default();
            written.by_id(id).map(|found| found.name.as_str())
        })
        .collect::<Result<_, _>>()?;
    assert_eq!(described, ["hull", "decal", "canopy", "mystery"]);
    for material in written
        .manifest
        .materials
        .iter()
        .filter(|material| material.index.is_some())
    {
        assert_eq!(material.library, "Objects/Test/layered");
    }
    Ok(())
}

#[test]
fn a_layered_material_carries_its_shader_flags_colours_and_parameters() -> TestResult {
    let written = write()?;
    let hull = written.material("hull")?;

    assert_eq!(hull.shader, "HardSurface");
    assert_eq!(hull.surface_type, "metal_solid");
    assert_eq!(
        hull.gen_mask,
        ["VERTCOLORS", "USE_DAMAGE_MAP", "WEAR_LAYER"]
    );
    assert_eq!(hull.index, Some(1));
    assert_eq!(hull.diffuse, [0.5; 3]);
    assert_eq!(hull.alpha_mode, AlphaMode::Opaque);
    assert!(hull.reads_vertex_colours, "layers without a blend map");
    assert!(!written.material("canopy")?.reads_vertex_colours);
    assert_eq!(hull.palette, Some(PaletteEntry::Primary));
    assert_eq!(
        hull.default_palette.as_deref(),
        Some("Libs/Foundry/Records/TintPalettes/brand/test/p_layered")
    );
    assert_eq!(parameter(hull, "WearBlendBase"), Some("0.25"));
    let names: Vec<&str> = hull.parameters.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["WearBlendBase", "DamageTiling"], "in file order");
    Ok(())
}

#[test]
fn layers_carry_tints_tiling_palette_slots_and_the_material_they_name() -> TestResult {
    let written = write()?;
    let hull = written.material("hull")?;

    let [primary, wear, lost, looped] = hull.layers.as_slice() else {
        return Err(format!("{} layers", hull.layers.len()).into());
    };
    assert_eq!(primary.name, "Primary");
    assert_eq!(primary.path, "Objects/Test/layers/base.mtl");
    assert_eq!(primary.sub_material, None);
    assert_eq!(primary.tint_colour, Some([0.75, 0.5, 0.25]));
    assert_eq!(primary.wear_tint, Some([1.0; 3]));
    assert_eq!(primary.gloss_mult, Some(0.5));
    assert_eq!(primary.uv_tiling, Some(2.0));
    assert_eq!(primary.palette, Some(PaletteEntry::Primary));
    let names: Vec<&str> = primary.attributes.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Name",
            "Path",
            "Submtl",
            "TintColor",
            "WearTint",
            "GlossMult",
            "UVTiling",
            "PaletteTint"
        ]
    );
    assert_eq!(wear.tint_colour, None, "a malformed colour is not parsed");
    assert_eq!(
        wear.attributes
            .iter()
            .find(|p| p.name == "TintColor")
            .map(|p| p.value.as_str()),
        Some("x"),
        "but it stays as written"
    );
    assert_eq!(wear.palette, None);
    assert_eq!(lost.material, None);
    assert!(looped.material.is_some());
    Ok(())
}

#[test]
fn a_layer_is_described_as_a_material_of_its_own_and_referenced_by_id() -> TestResult {
    let written = write()?;
    let hull = written.material("hull")?;
    let [primary, wear, _, looped] = hull.layers.as_slice() else {
        return Err(format!("{} layers", hull.layers.len()).into());
    };

    let base = written.by_id(primary.material.as_deref().ok_or("base")?)?;
    assert_eq!(base.library, "Objects/Test/layers/base");
    assert_eq!(base.index, None, "a layer file is one material");
    assert_eq!(base.shader, "Layer");
    assert_eq!(parameter(base, "TintMode"), Some("0"));
    let uses: Vec<(&str, TextureUse)> = base
        .textures
        .iter()
        .map(|texture| (texture.slot.as_str(), texture.usage))
        .collect();
    assert_eq!(
        uses,
        [
            ("TexSlot1", TextureUse::Diffuse),
            ("TexSlot2", TextureUse::Normal),
            ("TexSlot7", TextureUse::Detail),
            ("TexSlot8", TextureUse::Blend)
        ]
    );
    assert!(
        base.textures
            .iter()
            .all(|texture| texture.texture.is_some())
    );

    let worn = written.by_id(wear.material.as_deref().ok_or("wear")?)?;
    assert_eq!(worn.textures.len(), 3);
    assert_eq!(
        worn.textures.last().map(|t| t.usage),
        Some(TextureUse::Specular)
    );
    assert_eq!(worn.textures.last().and_then(|t| t.texture.clone()), None);

    let looping = written.by_id(looped.material.as_deref().ok_or("loop")?)?;
    assert_eq!(
        looping
            .layers
            .first()
            .and_then(|layer| layer.material.as_deref()),
        Some(looping.id.as_str()),
        "a layer that names its own file refers to it"
    );
    Ok(())
}

#[test]
fn a_layer_file_that_is_missing_is_an_issue() -> TestResult {
    let written = write()?;

    let issue = written
        .manifest
        .issues
        .iter()
        .find(|issue| issue.kind == "layer_material")
        .ok_or("issue")?;
    assert_eq!(issue.origin, IssueOrigin::Material);
    assert_eq!(issue.path, "Objects/Test/layers/lost.mtl");
    assert_eq!(issue.name, "hull");
    assert!(!issue.skipped);
    let texture_issue = written
        .manifest
        .issues
        .iter()
        .find(|issue| issue.origin == IssueOrigin::Texture)
        .ok_or("texture issue")?;
    assert_eq!(texture_issue.path, LAYERED_MISSING_TEXTURE);
    Ok(())
}

#[test]
fn decals_keep_their_placeholders_uv_modifiers_and_stencil_parameters() -> TestResult {
    let written = write()?;
    let decal = written.material("decal")?;

    assert_eq!(decal.shader, "MeshDecal");
    assert_eq!(decal.alpha_mode, AlphaMode::Blend);
    assert_eq!(parameter(decal, "StencilDiffuseColor"), Some("0.1,0.2,0.3"));
    let stated = decal.stencil.ok_or("a stencil")?;
    assert_eq!(stated.colour, Some([0.1, 0.2, 0.3]));
    assert!((stated.opacity - 1.0).abs() < f32::EPSILON);
    let [stencil, diffuse] = decal.textures.as_slice() else {
        return Err("two textures".into());
    };
    assert_eq!(stencil.file, "$TintPaletteDecal");
    assert_eq!(stencil.texture, None, "a placeholder is no file");
    assert_eq!(diffuse.usage, TextureUse::Diffuse);
    assert!(diffuse.texture.is_some());
    let tex_mod = diffuse.elements.first().ok_or("TexMod")?;
    assert_eq!(tex_mod.name, "TexMod");
    let attributes: Vec<(&str, &str)> = tex_mod
        .attributes
        .iter()
        .map(|p| (p.name.as_str(), p.value.as_str()))
        .collect();
    assert_eq!(attributes, [("TileU", "3"), ("OffsetV", "0.5")]);
    assert!(diffuse.attributes.iter().any(|p| p.name == "TexType"));
    Ok(())
}

#[test]
fn glass_takes_the_glass_entry_and_blends() -> TestResult {
    let written = write()?;
    let canopy = written.material("canopy")?;

    assert_eq!(canopy.palette, Some(PaletteEntry::Glass));
    assert_eq!(canopy.alpha_mode, AlphaMode::Blend);
    assert_eq!(canopy.gen_mask, ["NORMAL_MAP", "USE_TINT_PALETTE"]);
    Ok(())
}

#[test]
fn unknown_shader_parameters_attributes_and_elements_are_kept_raw() -> TestResult {
    let written = write()?;
    let mystery = written.material("mystery")?;

    assert_eq!(mystery.shader, "ZedPBR");
    assert_eq!(mystery.alpha_mode, AlphaMode::Mask);
    assert_eq!(parameter(mystery, "ZedStrength"), Some("1.5"));
    assert_eq!(parameter(mystery, "ZedMode"), Some("odd"));
    assert!(
        mystery
            .attributes
            .iter()
            .any(|p| p.name == "ZedFlag" && p.value == "7")
    );
    let [element] = mystery.elements.as_slice() else {
        return Err("one element".into());
    };
    assert_eq!(element.name, "ZedExtension");
    assert_eq!(
        element.attributes.first().map(|p| p.value.as_str()),
        Some("3")
    );
    assert_eq!(
        element.children.first().map(|c| c.name.as_str()),
        Some("Inner")
    );
    Ok(())
}

#[test]
fn a_palette_is_written_once_and_the_mesh_references_it() -> TestResult {
    let written = write()?;

    let [palette] = written.manifest.palettes.as_slice() else {
        return Err("one palette".into());
    };
    assert_eq!(palette.name, "p_layered");
    let mesh = written.manifest.meshes.first().ok_or("mesh")?;
    assert_eq!(mesh.palette_id.as_deref(), Some(palette.id.as_str()));
    assert_eq!(mesh.palette.as_deref(), Some("p_layered"));
    for (entry, bytes) in [palette.primary, palette.secondary, palette.tertiary]
        .iter()
        .zip(LAYERED_TINTS)
    {
        for (channel, byte) in entry.tint.iter().zip(bytes) {
            assert!((channel - srgb8_to_linear(byte)).abs() < 1e-5);
        }
    }
    Ok(())
}

#[test]
fn every_texture_slot_is_written_once_with_the_colour_space_of_its_use() -> TestResult {
    let written = write()?;
    let textures = &written.manifest.textures;

    let source = |name: &str| textures.iter().find(|t| t.source.ends_with(name));
    let diffuse = source("base_diff.dds").ok_or("diffuse")?;
    assert_eq!(diffuse.colour_space, ColourSpace::Srgb);
    let normal = source("base_ddna.dds").ok_or("normal")?;
    assert_eq!(normal.colour_space, ColourSpace::Linear);
    assert_eq!(normal.alpha, AlphaMeaning::None);
    let blend = source("base_blend.dds").ok_or("blend")?;
    assert_eq!(blend.colour_space, ColourSpace::Linear);
    assert!(
        source("wear_spec.dds").is_none(),
        "the missing file is not written"
    );
    let mut sources: Vec<&str> = textures.iter().map(|t| t.source.as_str()).collect();
    sources.sort_unstable();
    sources.dedup();
    assert_eq!(sources.len(), textures.len(), "each texture once");
    assert_eq!(textures.len(), 8);
    Ok(())
}

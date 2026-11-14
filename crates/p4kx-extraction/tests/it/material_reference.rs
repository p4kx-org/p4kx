// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Archive material-reference mapping and the resulting GLB material bindings.

use std::fs;

use p4kx_extraction::{AnimationSet, GlbOptions, MaterialMode, ModelInfo, ModelSource};
use p4kx_testkit::gltf::material::AlphaMode;
use p4kx_testkit::{MATERIAL_REFERENCE_MODEL, TestEntry, TestMtl, TestMtlMaterial, check_glb};

use super::material_reference_support::{TARGET, TestResult, archive as fixture_archive};

#[test]
fn admitted_references_load_the_library_and_assign_its_materials() -> TestResult {
    for reference in [
        "/Objects/Test/paint.mtl",
        "/Data/Objects/Test/paint.mtl",
        "//authoring-host/branch/Data/Objects/Test/paint.mtl",
        "//authoring-host/work/branch/Data/Objects/Test/paint.mtl",
        r"//authoring-host/branch/dAtA\Objects/Test\PAINT.MTL",
        "Objects/Test/paint",
        "Data/Objects/Test/paint.mtl",
        r"data\objects/TEST\PAINT.MTL",
    ] {
        let folder = tempfile::tempdir()?;
        let archive = fixture_archive(folder.path(), reference, Some(TARGET), [])?;
        let source = ModelSource::from_archive(&archive, MATERIAL_REFERENCE_MODEL)?;
        let library = source
            .model()
            .materials
            .library()
            .ok_or("library not loaded")?;
        assert_eq!(library.path, reference, "original spelling");
        assert_eq!(
            library.target.as_deref(),
            Some(TARGET),
            "physical Entry spelling"
        );
        assert_eq!(
            source.model().materials.problem(),
            None,
            "loaded without a problem"
        );
        let info = serde_json::to_value(ModelInfo::of(&source, &AnimationSet::default()))?;
        assert_eq!(info["library"]["path"], reference, "JSON source spelling");
        assert_eq!(info["library"]["target"], TARGET, "JSON physical target");

        let path = folder.path().join("cube.glb");
        let options = GlbOptions::default().with_materials(MaterialMode::Colours);
        let summary = source.write_glb(&options, &path)?;
        let glb = check_glb(&fs::read(path)?)?;
        assert_eq!(summary.default_material_ids, 0, "{reference}: no fallback");
        assert_eq!(summary.materials, 2, "only the two used materials");
        assert_eq!(summary.primitives, 2, "both authored submeshes");
        let mesh = glb.document.meshes().next().ok_or("mesh")?;
        let materials: Vec<_> = mesh.primitives().map(|part| part.material()).collect();
        let [caps, sides] = materials.as_slice() else {
            return Err("expected two primitive materials".into());
        };
        assert_eq!(caps.name(), Some("caps"), "caps material ID");
        assert_eq!(sides.name(), Some("sides"), "sides material ID");
        assert_ne!(caps.index(), sides.index(), "distinct material bindings");
        assert_colour(
            caps.pbr_metallic_roughness().base_color_factor(),
            [0.8, 0.1, 0.1, 1.0],
        );
        assert_colour(
            sides.pbr_metallic_roughness().base_color_factor(),
            [0.2, 0.4, 0.6, 0.5],
        );
        assert_eq!(caps.alpha_mode(), AlphaMode::Opaque, "opaque caps");
        assert_eq!(sides.alpha_mode(), AlphaMode::Blend, "translucent sides");
    }
    Ok(())
}

#[test]
fn physical_target_spelling_and_bare_sibling_precedence_are_preserved() -> TestResult {
    let folder = tempfile::tempdir()?;
    let physical = r"dAtA\Objects\TEST\PaInT.MTL";
    let fallback = TestMtl::single(&TestMtlMaterial::new("fallback")).cryxml();
    let archive = fixture_archive(
        folder.path(),
        "paint",
        Some(physical),
        [TestEntry::stored("Data/paint.mtl", fallback)],
    )?;
    let source = ModelSource::from_archive(&archive, MATERIAL_REFERENCE_MODEL)?;
    let library = source
        .model()
        .materials
        .library()
        .ok_or("sibling not loaded")?;
    assert_eq!(
        library.target.as_deref(),
        Some(physical),
        "physical target spelling"
    );
    assert_eq!(library.materials.len(), 3, "sibling wins over Data root");

    let fallback_folder = tempfile::tempdir()?;
    let archive = fixture_archive(fallback_folder.path(), "paint", Some("Data/paint.mtl"), [])?;
    let source = ModelSource::from_archive(&archive, MATERIAL_REFERENCE_MODEL)?;
    assert_eq!(
        source.model().materials.target(),
        Some("Data/paint.mtl"),
        "Data fallback target"
    );
    Ok(())
}

fn assert_colour(actual: [f32; 4], expected: [f32; 4]) {
    assert!(
        actual
            .iter()
            .zip(expected)
            .all(|(a, e)| (a - e).abs() < 1e-5),
        "{actual:?} differs from {expected:?}"
    );
}

#[test]
fn a_rooted_bare_name_skips_a_matching_sibling_library() -> TestResult {
    for reference in ["/paint.mtl", "//authoring-host/branch/Data/paint.mtl"] {
        let folder = tempfile::tempdir()?;
        let sibling = TestMtl::single(&TestMtlMaterial::new("sibling")).cryxml();
        let archive = fixture_archive(
            folder.path(),
            reference,
            Some("Data/paint.mtl"),
            [TestEntry::stored(TARGET, sibling)],
        )?;
        let source = ModelSource::from_archive(&archive, MATERIAL_REFERENCE_MODEL)?;
        let library = source.model().materials.library().ok_or("root library")?;
        assert_eq!(
            library.target.as_deref(),
            Some("Data/paint.mtl"),
            "rooted target"
        );
        assert_eq!(
            library.materials.len(),
            3,
            "rooted library wins over sibling"
        );
    }
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx model glb --materials colours` and the materials of `p4kx model info`, end to end; the
//! textures are in `model_textures.rs`.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, TestEntry, TestModel, TestMtl, TestMtlMaterial, check_glb,
    gltf::material::AlphaMode,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const MODEL_ENTRY: &str = "Data\\Objects\\Test\\cube.cgf";
const GEOMETRY_ENTRY: &str = "Data\\Objects\\Test\\cube.cgfm";
const MATERIAL_ENTRY: &str = "Data\\objects\\test\\cube.mtl";

fn mtl() -> TestMtl {
    TestMtl::multi(&[
        TestMtlMaterial::new("unused"),
        TestMtlMaterial::new("caps").diffuse([0.8, 0.1, 0.1]),
        TestMtlMaterial::new("sides")
            .diffuse([0.1, 0.1, 0.8])
            .opacity(0.5),
    ])
}

fn archive(
    folder: &Path,
    model: &TestModel,
    mtl: Option<Vec<u8>>,
) -> Result<PathBuf, Box<dyn Error>> {
    let path = folder.join("Data.p4k");
    let mut builder = ArchiveBuilder::new()
        .entry(TestEntry::zstd(MODEL_ENTRY, model.cgf()))
        .entry(TestEntry::zstd(GEOMETRY_ENTRY, model.cgfm()));
    if let Some(mtl) = mtl {
        builder = builder.entry(TestEntry::zstd(MATERIAL_ENTRY, mtl));
    }
    builder.write_to(&path)?;
    Ok(path)
}

fn glb(archive: &Path, out: &Path, extra: &[&str]) -> Result<std::process::Output, Box<dyn Error>> {
    Ok(p4kx_testkit::p4kx()?
        .args(["model", "glb"])
        .arg(archive)
        .arg(MODEL_ENTRY)
        .arg("--out")
        .arg(out)
        .args(extra)
        .output()?)
}

#[test]
fn colours_mode_writes_one_material_per_sub_material_with_colour_and_opacity_and_no_textures()
-> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path(), &TestModel::cube(), Some(mtl().cryxml()))?;
    let out = dir.path().join("cube.glb");

    let output = glb(&archive, &out, &["--materials", "colours"])?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("2 materials"), "summary: {stdout}");
    let glb = check_glb(&fs::read(&out)?)?;
    assert_eq!(glb.document.textures().len(), 0, "no textures");
    assert_eq!(glb.document.images().len(), 0, "no images");
    let colours: Vec<(Option<&str>, [f32; 4], AlphaMode)> = glb
        .document
        .materials()
        .map(|m| {
            (
                m.name(),
                m.pbr_metallic_roughness().base_color_factor(),
                m.alpha_mode(),
            )
        })
        .collect();
    assert_eq!(
        colours,
        [
            (Some("caps"), [0.8, 0.1, 0.1, 1.0], AlphaMode::Opaque),
            (Some("sides"), [0.1, 0.1, 0.8, 0.5], AlphaMode::Blend),
        ],
        "caps are ID 1 and sides ID 2"
    );
    Ok(())
}

#[test]
fn colours_is_the_default_and_an_unknown_mode_is_refused() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path(), &TestModel::cube(), Some(mtl().cryxml()))?;

    let default = glb(&archive, &dir.path().join("a.glb"), &[])?;
    let unknown = glb(&archive, &dir.path().join("b.glb"), &["--materials", "pbr"])?;

    assert_eq!(default.status.code(), Some(0), "colours by default");
    assert!(
        !String::from_utf8_lossy(&default.stdout).contains("embedded"),
        "no textures"
    );
    assert_eq!(
        unknown.status.code(),
        Some(2),
        "an unknown mode is invalid input"
    );
    Ok(())
}

#[test]
fn a_missing_mtl_is_reported_and_the_model_exports_with_a_default_material() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path(), &TestModel::cube(), None)?;
    let out = dir.path().join("cube.glb");

    let output = glb(&archive, &out, &[])?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        stdout.contains(
            "the material library objects/test/cube mapped to Data/objects/test/cube.mtl does not exist; the default material is used"
        ),
        "reported: {stdout}"
    );
    let glb = check_glb(&fs::read(&out)?)?;
    let names: Vec<Option<&str>> = glb.document.materials().map(|m| m.name()).collect();
    assert_eq!(names, [Some("default")], "one default material");
    assert_eq!(glb.document.meshes().len(), 1, "the geometry is there");

    let json = p4kx_testkit::p4kx()?
        .args(["--json", "model", "glb"])
        .arg(&archive)
        .arg(MODEL_ENTRY)
        .arg("--out")
        .arg(dir.path().join("again.glb"))
        .output()?;
    let summary: Value = serde_json::from_slice(&json.stdout)?;
    assert_eq!(summary["library"]["status"], "missing", "status");
    assert_eq!(summary["library"]["path"], "objects/test/cube", "path");
    assert_eq!(
        summary["library"]["target"], "Data/objects/test/cube.mtl",
        "mapped target"
    );
    assert_eq!(summary["default_material_ids"], 2, "both IDs");
    assert!(summary["library"]["problem"].is_string(), "problem text");
    Ok(())
}

#[test]
fn a_corrupt_mtl_is_reported_and_the_model_still_exports() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path(), &TestModel::cube(), Some(b"<Material".to_vec()))?;
    let out = dir.path().join("cube.glb");

    let output = glb(&archive, &out, &[])?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("cannot be read"), "reported: {stdout}");
    check_glb(&fs::read(&out)?)?;
    Ok(())
}

#[test]
fn submeshes_that_draw_nothing_are_left_out_and_reported() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mtl = TestMtl::multi(&[
        TestMtlMaterial::new("unused"),
        TestMtlMaterial::new("caps"),
        TestMtlMaterial::new("proxy").shader("NoDraw"),
    ]);
    let archive = archive(dir.path(), &TestModel::cube(), Some(mtl.cryxml()))?;
    let out = dir.path().join("cube.glb");

    let output = glb(&archive, &out, &[])?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        stdout.contains("left out 1 submeshes whose material draws nothing"),
        "reported: {stdout}"
    );
    assert_eq!(check_glb(&fs::read(&out)?)?.document.materials().len(), 1);
    Ok(())
}

#[test]
fn a_library_path_that_leaves_the_data_folder_is_refused() -> TestResult {
    let dir = tempfile::tempdir()?;
    let model = TestModel::cube().with_material_library("../../secret");
    let archive = archive(dir.path(), &model, Some(mtl().cryxml()))?;
    let out = dir.path().join("cube.glb");

    let output = glb(&archive, &out, &[])?;

    assert_eq!(
        output.status.code(),
        Some(0),
        "the model still exports: {output:?}"
    );
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("cannot be read"), "reported: {stdout}");
    assert!(stdout.contains("../../secret"), "named: {stdout}");
    Ok(())
}

#[test]
fn a_file_model_never_reads_a_library_outside_its_folder() -> TestResult {
    let dir = tempfile::tempdir()?;
    let folder = dir.path().join("models");
    fs::create_dir(&folder)?;
    let model = TestModel::cube().with_material_library("../secret");
    fs::write(folder.join("cube.cgf"), model.cgf())?;
    fs::write(folder.join("cube.cgfm"), model.cgfm())?;
    fs::write(dir.path().join("secret.mtl"), mtl().cryxml())?;

    let output = p4kx_testkit::p4kx()?
        .current_dir(&folder)
        .args(["--json", "model", "info", "cube.cgf"])
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let info: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(info["library"]["status"], "unreadable", "refused");
    assert_eq!(info["materials"], serde_json::json!([]), "nothing read");
    Ok(())
}

#[test]
fn info_lists_the_materials_with_shader_colours_and_texture_slots() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mtl = TestMtl::multi(&[
        TestMtlMaterial::new("unused"),
        TestMtlMaterial::new("caps")
            .diffuse([0.5, 0.25, 0.125])
            .surface_type("metal_dense")
            .texture("TexSlot1", "Objects/Test/caps_diff.tif"),
        TestMtlMaterial::new("sides"),
    ])
    .with_default_palette("Libs/Palette/test");
    let archive = archive(dir.path(), &TestModel::cube(), Some(mtl.cryxml()))?;

    let output = p4kx_testkit::p4kx()?
        .args(["--json", "model", "info"])
        .arg(&archive)
        .arg(MODEL_ENTRY)
        .output()?;

    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    let info: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(info["library"]["status"], "loaded", "status");
    assert_eq!(info["library"]["single"], false, "a list");
    assert_eq!(info["library"]["default_palette"], "Libs/Palette/test");
    let caps = &info["materials"][1];
    assert_eq!(caps["id"], 1, "ID");
    assert_eq!(caps["name"], "caps", "name");
    assert_eq!(caps["shader"], "Illum", "shader");
    assert_eq!(caps["surface_type"], "metal_dense", "surface");
    assert_eq!(
        caps["diffuse"],
        serde_json::json!([0.5, 0.25, 0.125]),
        "diffuse"
    );
    assert_eq!(
        caps["textures"],
        serde_json::json!([{"map": "TexSlot1", "file": "Objects/Test/caps_diff.tif"}]),
        "texture slots"
    );
    Ok(())
}

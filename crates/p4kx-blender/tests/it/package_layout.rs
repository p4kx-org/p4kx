// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The layout of a package written from the synthetic ship `TEST_Livery`: files, manifest,
//! schema, shared meshes and textures.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use p4kx_blender::{
    AlphaMeaning, Basis, BuildInfo, ColourSpace, FORMAT_VERSION, Format, IssueOrigin, Manifest,
    NodeKind, PackageInfo, TextureFormat, Unit, manifest_schema, write_package,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{check_glb, livery_entries};

use crate::support::{TestResult, assemble, fixture, glb_options, names_in, never};

struct Written {
    _dir: tempfile::TempDir,
    out: PathBuf,
    manifest: Manifest,
    raw: serde_json::Value,
    summary: p4kx_blender::PackageSummary,
}

fn write(mip: u32) -> Result<Written, Box<dyn std::error::Error>> {
    let fixture = fixture(livery_entries())?;
    let archive = Archive::open(&fixture.path)?;
    let assembly = assemble(&archive, "TEST_Livery")?;
    let models = p4kx_assembly::ArchiveModels::new(&archive);
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("ship");
    let info = PackageInfo::new(
        Some("LIVE".to_owned()),
        Some(BuildInfo {
            branch: "sc-alpha-4.0.0".to_owned(),
            version: "4.0.0".to_owned(),
            public_patch: None,
            change_number: "1".to_owned(),
            build_id: "2".to_owned(),
        }),
    );
    let summary = write_package(
        &assembly,
        &models,
        &models.textures(),
        &glb_options(mip),
        &info,
        &out,
        &never,
    )?;
    let text = std::fs::read_to_string(out.join("scene.json"))?;
    Ok(Written {
        manifest: serde_json::from_str(&text)?,
        raw: serde_json::from_str(&text)?,
        _dir: dir,
        out,
        summary,
    })
}

fn read(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(std::fs::read(path)?)
}

#[test]
fn the_package_is_a_folder_of_a_manifest_meshes_and_textures() -> TestResult {
    let written = write(2)?;

    assert_eq!(
        names_in(&written.out)?,
        [
            "material-sources",
            "meshes",
            "receipt.json",
            "scene.json",
            "textures"
        ],
        "nothing else is in the folder"
    );
    assert_eq!(
        names_in(&written.out.join("meshes"))?,
        ["m0001.glb", "m0002.glb", "m0003.glb", "m0004.glb"]
    );
    assert_eq!(names_in(&written.out.join("textures"))?, ["t0001.png"]);
    let mut expected = declared_files(&written.manifest);
    expected.push("receipt.json".into());
    assert_eq!(
        written.summary.files,
        expected.len(),
        "summary counts sidecars"
    );
    let on_disk = expected
        .iter()
        .map(|path| std::fs::metadata(written.out.join(path)).map(|metadata| metadata.len()))
        .sum::<Result<u64, _>>()?;
    assert_eq!(
        written.summary.bytes, on_disk,
        "the summary counts every file"
    );
    Ok(())
}

fn declared_files(manifest: &Manifest) -> Vec<String> {
    let mut paths = vec!["scene.json".to_owned()];
    paths.extend(manifest.meshes.iter().map(|mesh| mesh.file.clone()));
    paths.extend(manifest.textures.iter().map(|texture| texture.file.clone()));
    paths.extend(
        manifest
            .material_libraries
            .iter()
            .filter_map(|material| material.authored_file.as_ref())
            .map(|source| source.file.clone()),
    );
    paths.sort_unstable();
    paths
}

#[test]
fn independent_writes_have_the_same_receipt_with_every_declared_file() -> TestResult {
    let first = write(2)?;
    let second = write(2)?;
    let receipt = read(&first.out.join("receipt.json"))?;
    assert_eq!(receipt, read(&second.out.join("receipt.json"))?);
    let receipt: serde_json::Value = serde_json::from_slice(&receipt)?;
    let files = receipt["files"].as_array().ok_or("receipt files")?;
    assert_eq!(receipt["schema"], 1);
    assert_eq!(receipt["archive_build"], first.raw["source"]["build"]);
    assert_eq!(receipt["generator"]["name"], first.raw["generator"]["name"]);
    assert_eq!(
        files
            .iter()
            .map(|file| file["path"].as_str())
            .collect::<Vec<_>>(),
        declared_files(&first.manifest)
            .iter()
            .map(|path| Some(path.as_str()))
            .collect::<Vec<_>>(),
        "every declared file has a deterministic digest"
    );
    for row in files {
        let path = row["path"].as_str().ok_or("relative file")?;
        let mut digest = p4kx_extraction::OriginDigestWriter::new(std::io::sink());
        std::io::copy(&mut std::fs::File::open(first.out.join(path))?, &mut digest)?;
        assert_eq!(row["sha256"], digest.finish()?.sha256());
    }
    Ok(())
}

#[test]
fn the_manifest_states_version_source_and_axes() -> TestResult {
    let written = write(2)?;
    let manifest = &written.manifest;

    assert_eq!(manifest.format, Format::P4kxBlenderPackage);
    assert_eq!(manifest.version, FORMAT_VERSION);
    assert_eq!(manifest.generator.name, "p4kx");
    assert_eq!(manifest.source.channel.as_deref(), Some("LIVE"));
    assert_eq!(
        manifest
            .source
            .build
            .as_ref()
            .map(|build| build.version.as_str()),
        Some("4.0.0")
    );
    assert_eq!(manifest.source.entity.class_name, "TEST_Livery");
    assert_eq!(manifest.coordinates.basis, Basis::GltfYUp);
    assert_eq!(manifest.coordinates.unit, Unit::Metre);
    assert_eq!(manifest.options.texture_mip, 2);
    assert_eq!(written.raw["format"], "p4kx-blender-package");
    assert_eq!(written.raw["coordinates"]["basis"], "gltf_y_up");
    Ok(())
}

#[test]
fn the_manifest_matches_its_schema() -> TestResult {
    let written = write(0)?;
    let validator = jsonschema::draft202012::new(&manifest_schema())?;

    let violations: Vec<String> = validator
        .iter_errors(&written.raw)
        .take(5)
        .map(|error| format!("{} at {}", error, error.instance_path()))
        .collect();

    assert!(violations.is_empty(), "{violations:?}");
    Ok(())
}

#[test]
fn the_schema_rejects_what_a_reader_must_refuse() -> TestResult {
    let written = write(0)?;
    let validator = jsonschema::draft202012::new(&manifest_schema())?;
    let broken = |pointer: &str, value: serde_json::Value| {
        let mut raw = written.raw.clone();
        if let Some(slot) = raw.pointer_mut(pointer) {
            *slot = value;
        }
        !validator.is_valid(&raw)
    };

    assert!(broken("/format", "something-else".into()));
    assert!(broken("/version", "one".into()));
    assert!(broken("/version", "1".into()));
    assert!(broken("/coordinates/basis", "z_up".into()));
    assert!(broken("/coordinates/unit", "foot".into()));
    assert!(broken("/textures/0/format", "exr".into()));
    assert!(broken("/textures/0/colour_space", "aces".into()));
    assert!(broken("/textures/0/alpha", "mask".into()));
    assert!(broken("/nodes/0/kind", "portal".into()));
    Ok(())
}

#[test]
fn nodes_keep_the_hierarchy_and_names_of_the_assembly() -> TestResult {
    let written = write(0)?;
    let nodes = &written.manifest.nodes;

    let names: Vec<&str> = nodes.iter().map(|node| node.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "TEST_Livery",
            "hardpoint_painted/TEST_Painted",
            "hardpoint_twin/TEST_Painted",
            "hardpoint_default/TEST_Default",
            "hardpoint_lost/TEST_Lost",
            "hardpoint_child/TEST_Child",
            "hardpoint_plain/TEST_Plain",
        ]
    );
    assert!(nodes.iter().all(|node| node.kind == NodeKind::Entity));
    assert!(nodes.iter().all(|node| node.entity.is_some()));
    assert_eq!(nodes.first().and_then(|node| node.parent), None);
    assert!(
        nodes.iter().skip(1).all(|node| node.parent == Some(0)),
        "every part hangs below the ship"
    );
    assert_eq!(
        nodes.get(1).map(|node| node.path.as_str()),
        Some("hardpoint_painted")
    );
    assert!(
        nodes.iter().all(
            |node| node.scale.iter().all(|s| (s.abs() - 1.0).abs() < 1e-5)
                && (node.rotation.iter().map(|q| q * q).sum::<f32>() - 1.0).abs() < 1e-4
        ),
        "unit scale and unit quaternions"
    );
    Ok(())
}

#[test]
fn parts_of_one_model_library_and_palette_share_one_mesh() -> TestResult {
    let written = write(0)?;
    let manifest = &written.manifest;

    let mesh_of = |index: usize| {
        manifest
            .nodes
            .get(index)
            .and_then(|node| node.mesh.as_deref())
    };
    assert_eq!(
        manifest.meshes.len(),
        4,
        "ship, painted, default, the parts that inherited"
    );
    assert_eq!(mesh_of(1), mesh_of(2), "the two painted guns share a mesh");
    assert_eq!(
        mesh_of(4),
        mesh_of(5),
        "lost and child fall back to the ship palette"
    );
    assert_eq!(mesh_of(4), mesh_of(6), "and so does the plain one");
    let distinct: BTreeSet<_> = (0..7).filter_map(mesh_of).collect();
    assert_eq!(distinct.len(), 4);
    let instances: Vec<usize> = manifest.meshes.iter().map(|mesh| mesh.instances).collect();
    assert_eq!(instances, [1, 2, 1, 3]);
    assert_eq!(written.summary.drawn_nodes, 7);
    assert_eq!(written.summary.shared_meshes, 3);
    let palettes: Vec<Option<&str>> = manifest
        .meshes
        .iter()
        .map(|mesh| mesh.palette.as_deref())
        .collect();
    assert_eq!(
        palettes,
        [
            Some("p_ship"),
            Some("p_gun"),
            Some("p_ship"),
            Some("p_ship")
        ]
    );
    Ok(())
}

#[test]
fn every_mesh_is_a_valid_glb_and_its_material_indices_are_the_writers_order() -> TestResult {
    let written = write(0)?;

    for mesh in &written.manifest.meshes {
        let bytes = read(&written.out.join(&mesh.file))?;
        let checked = check_glb(&bytes)?;
        assert_eq!(mesh.bytes, u64::try_from(bytes.len())?, "{}", mesh.id);
        assert_eq!(checked.document.images().len(), 0, "textures stay outside");
        assert_eq!(checked.document.scenes().len(), 1);
        assert_eq!(checked.document.meshes().len(), 1, "{}", mesh.id);
        assert!(mesh.triangles > 0 && mesh.vertices > 0);
        assert!(mesh.geometry.is_some(), "{}", mesh.id);
        let in_file: Vec<(usize, Option<&str>)> = checked
            .document
            .materials()
            .map(|material| (material.index().unwrap_or(usize::MAX), material.name()))
            .collect();
        let listed: Vec<(usize, Option<&str>)> = mesh
            .materials
            .iter()
            .map(|material| (material.index, Some(material.name.as_str())))
            .collect();
        assert_eq!(listed, in_file, "{}", mesh.id);
        assert_eq!(listed.len(), 2, "caps and sides: {}", mesh.id);
    }
    Ok(())
}

#[test]
fn textures_are_written_once_at_the_chosen_mip_and_named_by_the_materials() -> TestResult {
    let written = write(2)?;
    let manifest = &written.manifest;

    assert_eq!(manifest.textures.len(), 1);
    let texture = manifest.textures.first().ok_or("texture")?;
    assert_eq!(texture.file, "textures/t0001.png");
    assert_eq!(texture.source, "Objects/Test/livery_diff.dds");
    assert_eq!((texture.format, texture.bit_depth), (TextureFormat::Png, 8));
    assert_eq!(texture.colour_space, ColourSpace::Srgb, "a diffuse map");
    assert_eq!(texture.alpha, AlphaMeaning::None, "DXT1 has no alpha");
    assert_eq!((texture.width, texture.height), (2, 2), "8 by 8 at mip 2");
    let decoder = png::Decoder::new(std::io::Cursor::new(read(
        &written.out.join(&texture.file),
    )?));
    let info = decoder.read_info()?;
    assert_eq!(
        (info.info().width, info.info().height),
        (2, 2),
        "the file is what the manifest says"
    );
    assert_eq!(written.summary.textures, 1);
    assert_eq!(written.summary.texture_pixels, 4);
    let used: Vec<Option<&str>> = manifest
        .meshes
        .iter()
        .flat_map(|mesh| &mesh.materials)
        .map(|material| material.textures.diffuse.as_deref())
        .collect();
    assert_eq!(used.len(), 8, "caps and sides of four meshes");
    assert!(used.iter().all(|id| *id == Some("t0001")), "{used:?}");
    Ok(())
}

#[test]
fn what_the_assembly_could_not_resolve_is_listed_as_issues() -> TestResult {
    let written = write(0)?;

    let kinds: Vec<&str> = written
        .manifest
        .issues
        .iter()
        .map(|issue| issue.kind.as_str())
        .collect();
    assert_eq!(kinds, ["palette", "palette"], "lost and child");
    assert!(
        written
            .manifest
            .issues
            .iter()
            .all(|issue| issue.origin == IssueOrigin::Assembly && !issue.skipped)
    );
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The material descriptions of `p4kx entity package` over the synthetic layered ship.

use std::error::Error;

use p4kx_testkit::layered_entries;

use crate::entity_package::{manifest, package, write_archive};

type TestResult = Result<(), Box<dyn Error>>;

fn pretty(manifest: &serde_json::Value, field: &str) -> Result<String, Box<dyn Error>> {
    Ok(serde_json::to_string_pretty(&manifest[field])?)
}

#[test]
fn the_package_describes_every_layered_material() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), layered_entries())?;
    let out = dir.path().join("layered");

    let run = package(&archive, "TEST_Layered", &out, &[], false)?;

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let manifest = manifest(&out)?;
    let validator = jsonschema::draft202012::new(&p4kx_blender::manifest_schema())?;
    assert!(validator.is_valid(&manifest));
    insta::assert_snapshot!("materials", pretty(&manifest, "materials")?);
    insta::assert_snapshot!("palettes", pretty(&manifest, "palettes")?);
    insta::assert_snapshot!(
        "mesh_materials",
        pretty(&manifest["meshes"][0], "materials")?
    );
    Ok(())
}

#[test]
fn the_summary_counts_the_materials_the_layers_and_the_palettes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), layered_entries())?;
    let out = dir.path().join("layered");

    let run = package(&archive, "TEST_Layered", &out, &[], true)?;

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let summary: serde_json::Value = serde_json::from_str(&run.stdout)?;
    assert_eq!(summary["materials"], 7);
    assert_eq!(summary["palettes"], 1);
    Ok(())
}

#[test]
fn the_optional_dds_container_keeps_default_png_and_texture_ids() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), layered_entries())?;
    let png_folder = dir.path().join("png");
    let dds_folder = dir.path().join("dds");
    for (folder, options) in [
        (&png_folder, &[][..]),
        (&dds_folder, &["--texture-container", "dds"][..]),
    ] {
        let run = package(&archive, "TEST_Layered", folder, options, false)?;
        assert_eq!(run.code, Some(0), "{}", run.stderr);
    }
    let png = manifest(&png_folder)?;
    let dds = manifest(&dds_folder)?;
    let png = png["textures"].as_array().ok_or("PNG texture rows")?;
    let dds = dds["textures"].as_array().ok_or("DDS texture rows")?;
    assert_eq!(png.len(), dds.len(), "same texture count");
    assert!(
        png.iter().all(|row| row["format"] == "png"),
        "default stays PNG"
    );
    assert!(
        dds.iter().any(|row| row["format"] == "dds"),
        "compatible assets opt into DDS"
    );
    assert!(
        dds.iter()
            .any(|row| row["dds_fallback"] == "normal_coordinate_transform"),
        "normal map reports its channel transform"
    );
    for (plain, optional) in png.iter().zip(dds) {
        assert_eq!(plain["id"], optional["id"], "same stable texture id");
        assert_eq!(plain["width"], optional["width"], "same selected mip width");
        assert_eq!(
            plain["height"], optional["height"],
            "same selected mip height"
        );
    }
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A caller-selected installed source witness through ordinary material producers.

use std::error::Error;
use std::fs;
use std::path::PathBuf;

use p4kx_extraction::{Archive, ArchiveWatch, PathIndex};
use serde_json::json;

#[path = "installed_palette_subset_input.rs"]
mod input;
#[path = "installed_palette_subset_materials.rs"]
mod materials;
#[path = "installed_palette_subset_output.rs"]
mod output;
#[path = "installed_palette_subset_sources.rs"]
mod sources;

type Check<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_original_subset_preserves_material_and_uv_inputs() -> Check {
    run()
}

fn run() -> Check {
    let (request, pins) = input::load()?;
    let output = PathBuf::from(
        std::env::var_os("P4KX_INSTALLED_PALETTE_SUBSET_OUTPUT")
            .ok_or("P4KX_INSTALLED_PALETTE_SUBSET_OUTPUT is required")?,
    );
    if !output.is_absolute() || !output.is_dir() || fs::read_dir(&output)?.next().is_some() {
        return Err("a fresh empty absolute output directory is required".into());
    }
    let archive = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let watch = ArchiveWatch::new(&archive);
    request.check_archive(&archive)?;
    let index = PathIndex::new(&archive);
    let mut ledger = sources::Ledger::default();
    let palette = input::palette(&index, &request, &mut ledger)?;
    let (model, model_sources, uvs) = input::model(&index, &request, &mut ledger)?;
    let loaded = materials::Loaded::read(&index, &request, &mut ledger)?;
    watch.check()?;
    let view = output::View {
        model: &model,
        sources: &model_sources,
        loaded: &loaded,
        palette: &palette,
    };
    let result = output::write(&index, &request, &output, &view)?;
    sources::verify_output_sources(&index, &request, &result, &mut ledger)?;
    watch.check()?;
    request.check_archive(&archive)?;
    let verification_counts = ledger.counts()?;
    let receipt = json!({"schema":1,"scope":"one-original-root-subset-consumer-witness",
        "inputPins":pins,"glbProjection":result.glb_projection,"build":request.build,"lod":request.lod,"mesh":request.mesh,
        "subset":request.subset,"triangles":request.triangles,"mip":request.mip,
        "sourceUvProbes":uvs,"sources":ledger.rows,"verificationCounts":verification_counts,"logicalTextureRequests":result.requests,
        "normalPaletteParser":true,"paletteRecordIndex":request.palette_record,
        "paletteGuid":request.palette_guid,"paletteProvenanceInPackage":false,
        "modelAndMaterialClosureDecodedEstimate":request.closure_bytes,
        "dataforgeDecodedBytes":request.dataforge.decoded_bytes,
        "nativeBlenderReadbackExecuted":false,"gameAppearanceProven":false,
        "originalFirstUseVertexOrderPreserved":false,"firstUseVertexMapRequested":false,
        "runtimePermutation":null});
    fs::write(
        output.join("witness.private.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    Ok(())
}

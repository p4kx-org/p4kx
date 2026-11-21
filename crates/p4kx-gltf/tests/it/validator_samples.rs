// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Synthetic GLBs for the Khronos glTF-Validator: the CI job points `P4KX_VALIDATOR_DIR` at a
//! folder, this test writes the GLBs there and the job validates them.

use std::env;
use std::error::Error;
use std::path::PathBuf;

use p4kx_testkit::{TestMtlMaterial, check_glb};

use crate::morphs::{glb_bytes as rigged_bytes, target};
use crate::vertex_colour_primitives::{glass, glb_bytes as coloured_bytes};

type TestResult = Result<(), Box<dyn Error>>;

/// The folder the GLBs are written to, if the environment names one.
const FOLDER_VARIABLE: &str = "P4KX_VALIDATOR_DIR";

#[test]
fn the_synthetic_glbs_are_written_for_the_validator() -> TestResult {
    let head = rigged_bytes(&[
        target("smile", &[(2, [0.01, 0.02, 0.03]), (7, [-0.5, 0.0, 0.25])]),
        target("blink", &[(0, [0.0, 0.0, 0.001])]),
    ])?;
    let skinned = rigged_bytes(&[])?;
    let coloured = coloured_bytes(&[
        glass("unused", ""),
        glass("glass_ext", "%NORMAL_MAP"),
        glass("lens", "%PER_VERTEX_TINT"),
        TestMtlMaterial::new("plain"),
    ])?;
    let emissive = coloured_bytes(&[
        TestMtlMaterial::new("unused"),
        TestMtlMaterial::new("glow")
            .emissive([1.0, 0.5, 0.0])
            .glow(4.0),
        TestMtlMaterial::new("plain"),
    ])?;
    let samples = [
        ("emissive_strength.glb", emissive),
        ("head_with_morph_targets.glb", head),
        ("skinned_mesh.glb", skinned),
        ("per_primitive_color_0.glb", coloured),
    ];
    let folder = env::var_os(FOLDER_VARIABLE).map(PathBuf::from);
    if let Some(folder) = &folder {
        std::fs::create_dir_all(folder)?;
    }
    for (name, bytes) in &samples {
        check_glb(bytes)?;
        if let Some(folder) = &folder {
            std::fs::write(folder.join(name), bytes)?;
        }
    }
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;

pub(crate) struct Built {
    pub(crate) out: PathBuf,
    pub(crate) report: Value,
    pub(crate) manifest: Value,
    pub(crate) elapsed: Duration,
}

pub(crate) struct Measured {
    pub(crate) files: usize,
    pub(crate) bytes: u64,
    pub(crate) mesh_bytes: u64,
    pub(crate) meshes: usize,
    pub(crate) texture_sources: usize,
    pub(crate) textures: usize,
    pub(crate) instances: u64,
    pub(crate) undeduplicated_mesh_bytes: u64,
}

impl Measured {
    pub(crate) fn mesh_ratio(&self) -> f64 {
        ratio(self.mesh_bytes, self.undeduplicated_mesh_bytes)
    }
    pub(crate) fn sharing(&self) -> f64 {
        1.0 - ratio(self.meshes as u64, self.instances)
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "ratios report bounded package measurements"
)]
fn ratio(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        1.0
    } else {
        part as f64 / whole as f64
    }
}

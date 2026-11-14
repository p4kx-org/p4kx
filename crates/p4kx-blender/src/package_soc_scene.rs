// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::package_receipt::{self, PackageFile};
use crate::{PackageResult, PackageWriteContext, SOC_SCENE_VERSION, SocScene};

pub(crate) const DIRECTORY: &str = "authored";
pub(crate) const SCENE_FILE: &str = "authored/soc-scene.json";

/// An optional versioned authored SOC scene file in a package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ManifestSocScene {
    /// The fixed package-relative JSON file.
    #[schemars(extend("const" = SCENE_FILE))]
    pub file: String,
    /// The authored SOC scene schema version.
    #[schemars(extend("const" = SOC_SCENE_VERSION))]
    pub schema_version: u8,
}

impl ManifestSocScene {
    pub(crate) fn new() -> Self {
        Self {
            file: SCENE_FILE.to_owned(),
            schema_version: SOC_SCENE_VERSION,
        }
    }
}

pub(crate) fn has_expected_reference(reference: &ManifestSocScene) -> bool {
    reference.file == SCENE_FILE && reference.schema_version == SOC_SCENE_VERSION
}

pub(crate) fn write(
    staging: &Path,
    scene: &SocScene,
    context: &PackageWriteContext<'_>,
) -> PackageResult<PackageFile> {
    let _ = context.create_directory(&staging.join(DIRECTORY))?;
    let digest = package_receipt::write_json(&staging.join(SCENE_FILE), scene, context, false)?;
    Ok(PackageFile::new(SCENE_FILE.to_owned(), &digest))
}

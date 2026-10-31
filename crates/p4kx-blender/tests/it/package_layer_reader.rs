// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The package reads layer files through the provider's shared layer reader, not as libraries.

use std::sync::{Arc, Mutex, PoisonError};

use p4kx_assembly::{ArchiveModels, ModelFailure, ModelProvider, ProvidedModel};
use p4kx_blender::{PackageInfo, write_package};
use p4kx_extraction::Archive;
use p4kx_extraction::model::{LayerBudget, MaterialStatus};
use p4kx_testkit::layer_chain_entries;

use crate::support::{TestResult, assemble, fixture, glb_options, never};

type Calls = Mutex<Vec<String>>;

struct Recording<'a> {
    inner: &'a ArchiveModels<'a>,
    libraries: Calls,
    layers: Calls,
}

fn record(calls: &Calls, name: &str) {
    calls
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(name.to_owned());
}

fn recorded(calls: &Calls) -> Vec<String> {
    calls.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

impl ModelProvider for Recording<'_> {
    fn model(&self, path: &str, lod: u32) -> Result<ProvidedModel, ModelFailure> {
        self.inner.model(path, lod)
    }

    fn library(&self, entry: &str, name: &str) -> MaterialStatus {
        record(&self.libraries, name);
        self.inner.library(entry, name)
    }

    fn layer_library(&self, path: &str, budget: &mut LayerBudget) -> Arc<MaterialStatus> {
        record(&self.layers, path);
        self.inner.layer_library(path, budget)
    }
}

#[test]
fn layer_files_are_read_by_the_layer_reader_and_no_library_lookup() -> TestResult {
    let fixture = fixture(layer_chain_entries())?;
    let archive = Archive::open(&fixture.path)?;
    let assembly = assemble(&archive, "TEST_Chain")?;
    let inner = ArchiveModels::new(&archive);
    let models = Recording {
        inner: &inner,
        libraries: Calls::default(),
        layers: Calls::default(),
    };
    let dir = tempfile::tempdir()?;

    write_package(
        &assembly,
        &models,
        &inner.textures(),
        &glb_options(0),
        &PackageInfo::default(),
        &dir.path().join("ship"),
        &never,
    )?;

    let libraries = recorded(&models.libraries);
    assert!(
        libraries.iter().all(|name| !name.contains("/chain/")),
        "no layer file is looked up as a library: {libraries:?}"
    );
    assert!(
        recorded(&models.layers)
            .iter()
            .any(|path| path.contains("cyc_a")),
        "the layer reader is asked"
    );
    Ok(())
}

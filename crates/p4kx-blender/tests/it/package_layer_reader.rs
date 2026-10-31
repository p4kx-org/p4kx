// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The package reads layer files through the provider's shared layer reader, within the
//! provider's budget, and not as libraries.

use std::error::Error;
use std::sync::{Arc, Mutex, PoisonError};

use p4kx_assembly::{ArchiveModels, ModelFailure, ModelProvider, ProvidedModel};
use p4kx_blender::{IssueOrigin, Manifest, PackageInfo, write_package};
use p4kx_extraction::Archive;
use p4kx_extraction::model::{LayerBudget, LayerCache, LayerLimits, MaterialStatus};
use p4kx_testkit::layer_chain_entries;

use crate::support::{TestResult, assemble, fixture, glb_options, never};

type Calls = Mutex<Vec<String>>;

struct Recording<'a> {
    inner: &'a ArchiveModels<'a>,
    limits: LayerLimits,
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

    fn layer_budget(&self) -> LayerBudget {
        LayerCache::with_limits(self.limits).budget()
    }

    fn layer_library(&self, path: &str, budget: &mut LayerBudget) -> Arc<MaterialStatus> {
        record(&self.layers, path);
        self.inner.layer_library(path, budget)
    }
}

/// What a package of the chain ship came to: its manifest and the library and layer lookups the
/// package made of the provider.
struct Written {
    manifest: Manifest,
    libraries: Vec<String>,
    layers: Vec<String>,
}

fn package(limits: LayerLimits) -> Result<Written, Box<dyn Error>> {
    let fixture = fixture(layer_chain_entries())?;
    let archive = Archive::open(&fixture.path)?;
    let assembly = assemble(&archive, "TEST_Chain")?;
    let inner = ArchiveModels::new(&archive);
    let models = Recording {
        inner: &inner,
        limits,
        libraries: Calls::default(),
        layers: Calls::default(),
    };
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("ship");
    write_package(
        &assembly,
        &models,
        &inner.textures(),
        &glb_options(0),
        &PackageInfo::default(),
        &out,
        &never,
    )?;
    Ok(Written {
        manifest: serde_json::from_str(&std::fs::read_to_string(out.join("scene.json"))?)?,
        libraries: recorded(&models.libraries),
        layers: recorded(&models.layers),
    })
}

#[test]
fn layer_files_are_read_by_the_layer_reader_and_no_library_lookup() -> TestResult {
    let written = package(LayerLimits::default())?;

    let in_chain = |name: &String| {
        name.replace('\\', "/")
            .to_ascii_lowercase()
            .contains("/chain/")
    };
    assert!(
        !written.libraries.iter().any(in_chain),
        "no layer file is looked up as a library: {:?}",
        written.libraries
    );
    assert!(
        written.layers.iter().any(|path| path.contains("cyc_a")),
        "the layer reader is asked"
    );
    Ok(())
}

#[test]
fn a_package_whose_layer_budget_runs_out_reports_the_layers_it_did_not_read() -> TestResult {
    let written = package(LayerLimits {
        files_per_load: 3,
        ..LayerLimits::default()
    })?;

    let spent: Vec<_> = written
        .manifest
        .issues
        .iter()
        .filter(|issue| issue.kind == "layer_material" && issue.reason.contains("budget"))
        .collect();
    assert!(!spent.is_empty(), "{:?}", written.manifest.issues);
    assert!(
        spent
            .iter()
            .all(|issue| issue.origin == IssueOrigin::Material)
    );
    Ok(())
}

#[test]
fn the_provider_shares_one_layer_fetch_between_a_library_load_and_the_layer_reader() -> TestResult {
    let fixture = fixture(layer_chain_entries())?;
    let archive = Archive::open(&fixture.path)?;
    let models = ArchiveModels::new(&archive);

    drop(models.library("", "Objects/Test/chain"));
    let held = models.layer_cache().len();
    let mut budget = models.layer_budget();
    let first = models.layer_library("Objects/Test/chain/cyc_a.mtl", &mut budget);
    let second = models.layer_library("objects/test/chain/CYC_A", &mut budget);

    assert!(held >= 4, "the library load cached its layer files: {held}");
    assert_eq!(models.layer_cache().len(), held, "no file was read again");
    assert!(first.library().is_some(), "{first:?}");
    assert!(
        Arc::ptr_eq(&first, &second),
        "one library for two spellings"
    );
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Helpers of the Blender package verification: run `p4kx`, read the package back, check it.

#![expect(clippy::print_stdout, reason = "the measurements are the output")]

use std::error::Error;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use p4kx_testkit::{check_glb, real_archive_path};
use serde_json::Value;

pub(super) type TestResult = Result<(), Box<dyn Error>>;

/// A package as written, with what the CLI reported and what the folder holds.
pub(super) struct Built {
    pub(super) name: String,
    pub(super) out: PathBuf,
    pub(super) report: Value,
    pub(super) manifest: Value,
    pub(super) elapsed: Duration,
}

/// What the folder of a package holds.
pub(super) struct Measured {
    pub(super) files: usize,
    pub(super) bytes: u64,
    pub(super) mesh_bytes: u64,
    pub(super) meshes: usize,
    pub(super) instances: u64,
    pub(super) undeduplicated_mesh_bytes: u64,
}

impl Measured {
    pub(super) fn mesh_ratio(&self) -> f64 {
        ratio(self.mesh_bytes, self.undeduplicated_mesh_bytes)
    }

    pub(super) const fn textures(&self) -> usize {
        self.files.saturating_sub(self.meshes + 1)
    }

    pub(super) fn sharing(&self) -> f64 {
        1.0 - ratio(self.meshes as u64, self.instances)
    }
}

#[expect(clippy::cast_precision_loss, reason = "a ratio for a report")]
fn ratio(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        return 1.0;
    }
    part as f64 / whole as f64
}

/// The environment variable that keeps the packages after the checks passed.
pub(super) const KEEP_VAR: &str = "P4KX_KEEP_PACKAGES";

/// The folder of one test below `TMP`; it is removed when the test passes, unless
/// `P4KX_KEEP_PACKAGES` is set.
pub(super) struct Scratch(PathBuf);

impl Scratch {
    pub(super) fn new(test: &str) -> Result<Self, Box<dyn Error>> {
        let folder = std::env::temp_dir()
            .join("p4kx-package-verification")
            .join(test);
        std::fs::create_dir_all(&folder)?;
        Ok(Self(folder))
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if std::thread::panicking() || std::env::var_os(KEEP_VAR).is_some() {
            return;
        }
        let removed = std::fs::remove_dir_all(&self.0);
        assert!(removed.is_ok(), "{}: {removed:?}", self.0.display());
        if let Some(parent) = self.0.parent() {
            let emptied = std::fs::remove_dir(parent);
            drop(emptied);
        }
    }
}

/// Runs `p4kx <command> package <archive> [entity] --out <name> <extra>` and reads the package.
pub(super) fn build(
    scratch: &Scratch,
    command: &str,
    entity: Option<&str>,
    name: &str,
    extra: &[&OsStr],
) -> Result<Built, Box<dyn Error>> {
    let out = scratch.path().join(name);
    let mut run = p4kx_testkit::p4kx()?;
    run.env(
        "P4KX_ARCHIVE_KEY",
        p4kx_testkit::real_archive_key()?.to_hex(),
    );
    run.args(["--json", command, "package"])
        .arg(real_archive_path()?);
    if let Some(entity) = entity {
        run.arg(entity);
    }
    let started = Instant::now();
    let output = run.arg("--out").arg(&out).args(extra).output()?;
    let elapsed = started.elapsed();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{name}: {stderr}");
    let report: Value = serde_json::from_slice(&output.stdout)?;
    let manifest = serde_json::from_slice(&std::fs::read(out.join("scene.json"))?)?;
    Ok(Built {
        name: name.to_owned(),
        out,
        report,
        manifest,
        elapsed,
    })
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, Box<dyn Error>> {
    value[key]
        .as_array()
        .ok_or_else(|| format!("{key} is not an array").into())
}

fn files_below(folder: &Path) -> Result<Vec<(PathBuf, u64)>, Box<dyn Error>> {
    let mut found = Vec::new();
    let mut pending = vec![folder.to_owned()];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(next)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            if meta.is_dir() {
                pending.push(entry.path());
            } else {
                found.push((entry.path(), meta.len()));
            }
        }
    }
    Ok(found)
}

fn check_textures(built: &Built) -> TestResult {
    let mut sources = std::collections::HashSet::new();
    for texture in array(&built.manifest, "textures")? {
        let file = texture["file"].as_str().ok_or("texture file")?;
        assert!(
            built.out.join(file).is_file(),
            "{}: {file} exists",
            built.name
        );
        let source = texture["source"].as_str().ok_or("texture source")?;
        assert!(
            sources.insert(source.to_ascii_lowercase()),
            "{}: {source} is written twice",
            built.name
        );
    }
    Ok(())
}

fn check_layer_budget(built: &Built) -> TestResult {
    let spent: Vec<&Value> = array(&built.manifest, "issues")?
        .iter()
        .filter(|issue| {
            issue["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("budget"))
        })
        .collect();
    assert!(
        spent.is_empty(),
        "{}: the layer-file budget ran out: {spent:?}",
        built.name
    );
    Ok(())
}

/// Checks the package against the schema, its files against the manifest and every GLB, and
/// measures it.
pub(super) fn verify(built: &Built) -> Result<Measured, Box<dyn Error>> {
    let validator = jsonschema::draft202012::new(&p4kx_blender::manifest_schema())?;
    let violations: Vec<String> = validator
        .iter_errors(&built.manifest)
        .take(5)
        .map(|error| format!("{error} at {}", error.instance_path()))
        .collect();
    assert!(violations.is_empty(), "{}: {violations:?}", built.name);

    let on_disk = files_below(&built.out)?;
    let bytes: u64 = on_disk.iter().map(|(_, length)| length).sum();
    let mut measured = Measured {
        files: on_disk.len(),
        bytes,
        mesh_bytes: 0,
        meshes: 0,
        instances: 0,
        undeduplicated_mesh_bytes: 0,
    };
    for mesh in array(&built.manifest, "meshes")? {
        let file = mesh["file"].as_str().ok_or("mesh file")?;
        let content = std::fs::read(built.out.join(file))?;
        check_glb(&content).map_err(|error| format!("{}: {file}: {error}", built.name))?;
        let length = content.len() as u64;
        assert_eq!(
            mesh["bytes"].as_u64(),
            Some(length),
            "{}: {file}",
            built.name
        );
        let instances = mesh["instances"].as_u64().ok_or("instances")?;
        assert!(instances >= 1, "{}: {file} is drawn by no node", built.name);
        measured.meshes += 1;
        measured.mesh_bytes += length;
        measured.instances += instances;
        measured.undeduplicated_mesh_bytes += length * instances;
    }
    check_textures(built)?;
    check_layer_budget(built)?;
    for (key, counted) in [("files", measured.files as u64), ("bytes", bytes)] {
        assert_eq!(
            built.report.get(key).and_then(Value::as_u64),
            Some(counted),
            "{}: the report counts the {key}",
            built.name
        );
    }
    println!(
        "PACKAGE {}: {} files, {} bytes, {} meshes for {} drawing nodes (shared {:.3}) taking {} bytes ({:.3} of undeduplicated), {} textures, {} issues, {:.1?}",
        built.name,
        measured.files,
        measured.bytes,
        measured.meshes,
        measured.instances,
        measured.sharing(),
        measured.mesh_bytes,
        measured.mesh_ratio(),
        array(&built.manifest, "textures")?.len(),
        array(&built.manifest, "issues")?.len(),
        built.elapsed
    );
    Ok(measured)
}

/// Counts the nodes of a kind.
pub(super) fn nodes_of(manifest: &Value, kind: &str) -> Result<usize, Box<dyn Error>> {
    Ok(array(manifest, "nodes")?
        .iter()
        .filter(|node| node["kind"] == kind)
        .count())
}

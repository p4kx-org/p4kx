// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key, write_build_manifest};

use super::reference_digest;
use super::reference_layout;
use super::reference_load;
use super::reference_manifest::{
    Artifact, ArtifactRole, Completion, Expected, Identity, Location, MANIFEST_NAME, Manifest,
    PatchScope, SCHEMA_VERSION, Selection, SelectionMode, Tool,
};
use super::reference_prepare::{self, Plan, ProducerPlan};
use super::samples;
use super::unp4k_allow::PRODUCER_REVISION;

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

pub(crate) struct Fixture {
    pub(crate) archive: Archive,
    pub(crate) root: PathBuf,
    pub(crate) binary: PathBuf,
    _directory: tempfile::TempDir,
}

pub(crate) fn selected(path: &str, output: &str) -> Expected {
    Expected {
        identity: Identity::Entry {
            path: path.to_owned(),
        },
        targets: vec![Location {
            path: output.to_owned(),
            subresource: None,
        }],
    }
}

pub(crate) fn record(guid: &str, output: &str, pointer: Option<&str>) -> Expected {
    Expected {
        identity: Identity::Record {
            guid: guid.to_owned(),
            dcb: "Data/Game2.dcb".to_owned(),
        },
        targets: vec![Location {
            path: output.to_owned(),
            subresource: pointer.map(str::to_owned),
        }],
    }
}

pub(crate) fn selection(mode: SelectionMode) -> Selection {
    Selection {
        mode,
        declaration: "synthetic pre-conversion selection".to_owned(),
        empty_reason: None,
    }
}

pub(crate) const fn success() -> Completion {
    Completion {
        owner_asserts_success: true,
        exit_code: Some(0),
        errors: Vec::new(),
    }
}

pub(crate) fn freeze(manifest: &mut Manifest) -> AnyResult<()> {
    manifest.preparation.expected_digest = reference_layout::expected_digest(manifest)?;
    Ok(())
}

impl Fixture {
    pub(crate) fn new(entries: Vec<TestEntry>) -> AnyResult<Self> {
        let directory = tempfile::tempdir()?;
        let channel = directory.path().join("SYNTHETIC");
        let root = directory.path().join("reference");
        std::fs::create_dir_all(&channel)?;
        std::fs::create_dir_all(&root)?;
        write_build_manifest(
            &channel,
            "synthetic-branch",
            "1.0.0",
            "1",
            samples::SAMPLE_GUID,
        )?;
        let path = channel.join("Data.p4k");
        entries
            .into_iter()
            .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
            .write_to(&path)?;
        let archive = Archive::open_with_key(path, Some(test_key()))?;
        let binary = directory.path().join("synthetic-producer.bin");
        std::fs::write(&binary, b"synthetic producer identity")?;
        Ok(Self {
            _directory: directory,
            archive,
            root,
            binary,
        })
    }

    pub(crate) fn records() -> AnyResult<Self> {
        Self::new(vec![TestEntry::stored(
            "Data/Game2.dcb",
            samples::database(),
        )])
    }

    pub(crate) fn plan(
        tool: Tool,
        expected: Vec<Expected>,
        selection: Selection,
        payloads: &[&str],
    ) -> Plan {
        let mut artifacts: Vec<_> = payloads
            .iter()
            .map(|path| Artifact {
                path: (*path).to_owned(),
                role: ArtifactRole::Payload,
                fingerprint: None,
            })
            .collect();
        artifacts.extend([
            Artifact {
                path: "evidence/stdout.log".to_owned(),
                role: ArtifactRole::ProducerStdout,
                fingerprint: None,
            },
            Artifact {
                path: "evidence/stderr.log".to_owned(),
                role: ArtifactRole::ProducerStderr,
                fingerprint: None,
            },
            Artifact {
                path: "evidence/errors.json".to_owned(),
                role: ArtifactRole::ProducerErrorLog,
                fingerprint: None,
            },
        ]);
        Plan {
            schema_version: SCHEMA_VERSION,
            producer: ProducerPlan {
                tool,
                version: "1.0.0-synthetic".to_owned(),
                revision: if tool == Tool::Starbreaker {
                    "450fa284d4b1cbed6832675297dfb02579c0abd3"
                } else {
                    PRODUCER_REVISION
                }
                .to_owned(),
                patch_scope: PatchScope::Unmodified,
            },
            selection,
            expected,
            artifacts,
        }
    }

    pub(crate) fn write(&self, path: &str, bytes: &[u8]) -> AnyResult<()> {
        let path = self.root.join(path);
        std::fs::create_dir_all(path.parent().ok_or("output has no parent")?)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    pub(crate) fn save(&self, manifest: &Manifest) -> AnyResult<()> {
        std::fs::write(self.root.join(MANIFEST_NAME), serde_json::to_vec(manifest)?)?;
        Ok(())
    }

    fn logs(plan: &Plan) -> (String, String) {
        match plan.producer.tool {
            Tool::Unp4k => {
                let stdout = plan
                    .expected
                    .iter()
                    .filter_map(|source| {
                        if let Identity::Entry { path } = &source.identity {
                            Some(format!("Store | Plain | {path}\n"))
                        } else {
                            None
                        }
                    })
                    .collect();
                (stdout, String::new())
            }
            Tool::Unforge => (String::new(), String::new()),
            Tool::Starbreaker => {
                let count = plan
                    .artifacts
                    .iter()
                    .filter(|artifact| artifact.role == ArtifactRole::Payload)
                    .count();
                (
                    String::new(),
                    format!("DataCore loaded.\nExporting {count} records as json...\nDone.\n"),
                )
            }
        }
    }
    pub(crate) fn publish(&self, plan: Plan, payloads: &[(&str, Vec<u8>)]) -> AnyResult<Manifest> {
        let logs = Self::logs(&plan);
        reference_prepare::begin(&self.archive, &self.root, plan, &self.binary)?;
        for (path, bytes) in payloads {
            self.write(path, bytes)?;
        }
        self.write("evidence/stdout.log", logs.0.as_bytes())?;
        self.write("evidence/stderr.log", logs.1.as_bytes())?;
        reference_prepare::finalize(&self.archive, &self.root, &self.binary, success())?;
        Ok(reference_load::read(&self.root)?.manifest)
    }

    pub(crate) fn raw(&self, reference: &[u8]) -> AnyResult<Manifest> {
        let plan = Self::plan(
            Tool::Unp4k,
            vec![selected("Data/a.bin", "raw/a.bin")],
            selection(SelectionMode::Filtered),
            &["raw/a.bin"],
        );
        self.publish(plan, &[("raw/a.bin", reference.to_vec())])
    }

    pub(crate) fn rehash(&self, manifest: &mut Manifest, path: &str) -> AnyResult<()> {
        let artifact = manifest
            .artifacts
            .iter_mut()
            .find(|artifact| artifact.path == path)
            .ok_or("artifact missing")?;
        artifact.fingerprint = Some(reference_digest::file(&self.root.join(path), u64::MAX)?);
        Ok(())
    }
}

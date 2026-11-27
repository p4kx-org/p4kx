// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Path, PathBuf};

use p4kx_extraction::{
    Archive, ArchiveWatch, ArchiveWriteProtection, DataForgeEntry, DataForgeFileOrigin,
    DataForgeOrigin, InputScope, SourceInput, SourceLocation, SourceScope, SourceUseRole,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

use crate::checkpoint::Checkpoint;
use crate::{Capability, Projection, Provider, ProviderRegistry, SourceContext};
use crate::{ExportOptions, ExportOutcome, Index, MediaError, MediaKind, MediaWriter};
use crate::{generation_validation, hashing, origin_error};

pub(super) type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Debug, Clone, Copy)]
pub(super) enum Scene {
    Plain,
    EncryptedLeaf,
    EncryptedAncestor,
    UnknownAncestor,
    EncryptedOuterUnknownAncestor,
    EncryptedGrandparent,
    UnknownGrandparent,
}

pub(super) struct Fixture {
    pub(super) _folder: tempfile::TempDir,
    pub(super) archive: Archive,
    pub(super) output: PathBuf,
    pub(super) root: PathBuf,
    pub(super) index: Index,
    options: ExportOptions,
}

impl Fixture {
    pub(super) fn new(scene: Scene) -> Result<Self, Box<dyn std::error::Error>> {
        let folder = tempfile::tempdir()?;
        let archive = open_scene(folder.path(), scene)?;
        Self::export(folder, archive)
    }

    pub(super) fn from_builder(
        builder: &ArchiveBuilder,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let folder = tempfile::tempdir()?;
        let path = folder.path().join("Data.p4k");
        builder.write_to(&path)?;
        let archive = Archive::open_with(
            path,
            p4kx_extraction::OpenOptions::new().key(Some(p4kx_testkit::test_key())),
        )?;
        Self::export(folder, archive)
    }

    fn export(
        folder: tempfile::TempDir,
        archive: Archive,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let path = folder.path().join("standalone.dcb");
        fs::write(&path, b"synthetic standalone input")?;
        let file = DataForgeEntry::read_file(&path)?;
        let DataForgeOrigin::File(file) = file.origin() else {
            return Err("genuine standalone file observation".into());
        };
        let registry =
            ProviderRegistry::try_new(vec![Box::new(ScopeProbe { file: file.clone() })])?;
        let output = folder.path().join("media");
        let options = ExportOptions::default();
        let index = match crate::export_with(&archive, &output, &options, &registry)? {
            ExportOutcome::Published { index, .. } => *index,
            _ => return Err("genuine complete producer generation".into()),
        };
        let root = output.join(&index.artifact_base);
        Ok(Self {
            _folder: folder,
            archive,
            output,
            root,
            index,
            options,
        })
    }

    pub(super) fn reuse(&self) -> Result<Option<Index>, MediaError> {
        let watch = ArchiveWatch::new(&self.archive);
        let protection =
            ArchiveWriteProtection::new(self.archive.path(), self.options.cancel.flag())?;
        let checkpoint = Checkpoint {
            cancel: &self.options.cancel,
            watch: &watch,
            protection: &protection,
        };
        generation_validation::existing(&self.output, self.index.limits, &checkpoint)
    }

    pub(super) fn rebind(&mut self) -> TestResult {
        let watch = ArchiveWatch::new(&self.archive);
        let protection =
            ArchiveWriteProtection::new(self.archive.path(), self.options.cancel.flag())?;
        let checkpoint = Checkpoint {
            cancel: &self.options.cancel,
            watch: &watch,
            protection: &protection,
        };
        let generation = hashing::generation(&mut self.index, &checkpoint)?;
        let relative = format!("generations/{generation}");
        let destination = self.output.join(&relative);
        assert_ne!(
            destination, self.root,
            "the changed receipts have a fresh generation identity"
        );
        fs::rename(&self.root, &destination)?;
        self.root = destination;
        self.index.generation = generation;
        self.index.artifact_base = relative;
        fs::write(
            self.output.join("media-index.json"),
            serde_json::to_vec(&self.index)?,
        )?;
        assert_eq!(
            hashing::generation(&mut self.index, &checkpoint)?,
            self.index.generation,
            "the altered index is fully rebound before semantic verification"
        );
        Ok(())
    }
}

struct ScopeProbe {
    file: DataForgeFileOrigin,
}

impl Provider for ScopeProbe {
    fn capability(&self) -> Capability {
        Capability::new(
            MediaKind::Model,
            "synthetic.original-domains",
            "1.0.0",
            "retained sources",
        )
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        let input = context.origin_input(writer)?;
        domains(&input.scope)?;
        let unknown = writer
            .origins
            .owner
            .observe(SourceInput::Unknown, InputScope::Payload)
            .map_err(origin_error::evidence)?;
        domains(&unknown)?;
        let mut index = 0_usize;
        while let Some(ancestor) = input
            .scope
            .ancestor(index)
            .map_err(origin_error::evidence)?
        {
            domains(&ancestor)?;
            index = index
                .checked_add(1)
                .ok_or_else(|| MediaError::Contract("synthetic ancestor count overflow".into()))?;
        }
        let whole = writer
            .origins
            .root
            .as_ref()
            .ok_or_else(|| MediaError::Contract("actual completed root observation".into()))?;
        domains(&whole.scope)?;
        let file = writer
            .origins
            .owner
            .observe(SourceInput::File(&self.file), InputScope::Payload)
            .map_err(origin_error::evidence)?;
        domains(&file)?;
        Ok(Projection::Skipped)
    }
}

fn domains(scope: &SourceScope) -> Result<(), MediaError> {
    for domain in [
        InputScope::Payload,
        InputScope::ArchiveDirectoryMetadata,
        InputScope::FileMetadata,
    ] {
        let _observed = scope
            .for_scope(domain)
            .at(
                SourceLocation::Metadata {
                    field: "encryption-domain-probe".into(),
                },
                SourceUseRole::Reference,
            )
            .map_err(origin_error::evidence)?;
    }
    Ok(())
}

fn open_scene(folder: &Path, scene: Scene) -> Result<Archive, Box<dyn std::error::Error>> {
    let leaf = || TestEntry::stored("leaf.ext", b"original synthetic bytes");
    match scene {
        Scene::Plain => open_root(folder, leaf()),
        Scene::EncryptedLeaf => open_root(
            folder,
            TestEntry::zstd("leaf.ext", b"original synthetic bytes").encrypted(),
        ),
        Scene::EncryptedAncestor => {
            let bytes = ArchiveBuilder::new().entry(leaf()).build();
            open_root(
                folder,
                TestEntry::deflated("outer.socpak", bytes).encrypted(),
            )
        }
        Scene::UnknownAncestor => {
            let bytes = ArchiveBuilder::new().entry(leaf()).build();
            let root = open_root(folder, TestEntry::stored("outer.socpak", bytes))?;
            open_detached(&root)
        }
        Scene::EncryptedGrandparent | Scene::UnknownGrandparent => {
            let inner = ArchiveBuilder::new().entry(leaf()).build();
            let middle = ArchiveBuilder::new()
                .entry(TestEntry::stored("middle.socpak", inner))
                .build();
            if matches!(scene, Scene::EncryptedGrandparent) {
                open_root(
                    folder,
                    TestEntry::deflated("outer.socpak", middle).encrypted(),
                )
            } else {
                let root = open_root(folder, TestEntry::stored("outer.socpak", middle))?;
                open_detached(&root)
            }
        }
        Scene::EncryptedOuterUnknownAncestor => {
            let inner = ArchiveBuilder::new().entry(leaf()).build();
            let middle = ArchiveBuilder::new()
                .entry(TestEntry::stored("middle.socpak", inner))
                .build();
            let root = open_root(
                folder,
                TestEntry::deflated("outer.socpak", middle).encrypted(),
            )?;
            let outer =
                root.open_container(root.entries().first().ok_or("original outer Entry")?)?;
            open_detached(&outer)
        }
    }
}

fn open_detached(archive: &Archive) -> Result<Archive, Box<dyn std::error::Error>> {
    let detached = archive
        .entries()
        .first()
        .ok_or("original containing Entry")?
        .clone();
    Ok(archive.open_container(&detached)?)
}

fn open_root(folder: &Path, entry: TestEntry) -> Result<Archive, Box<dyn std::error::Error>> {
    let path = folder.join("Data.p4k");
    ArchiveBuilder::new().entry(entry).write_to(&path)?;
    Ok(Archive::open_with(
        path,
        p4kx_extraction::OpenOptions::new().key(Some(p4kx_testkit::test_key())),
    )?)
}

pub(super) fn rows(path: &Path) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    fs::read_to_string(path)?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

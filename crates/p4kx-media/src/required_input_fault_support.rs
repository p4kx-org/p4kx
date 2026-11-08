// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use p4kx_extraction::{
    Archive, ArchiveWatch, ArchiveWriteProtection, CancelToken, Exclusion, Filter, OpenOptions,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::checkpoint::Checkpoint;
use crate::directory_index::DirectoryIndex;
use crate::directory_inputs::DirectoryInputs;
use crate::directory_lookup::DirectoryLookup;
use crate::input_budget::InputBudget;
use crate::source_selection::SourceSelection;
use crate::staged_directory::StagedDirectory;
use crate::{Capability, MediaError, MediaKind, MediaWriter, SourceContext};

pub(crate) type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Copy)]
pub(crate) enum Inventory {
    Intact,
    Recovered,
}

pub(crate) fn with_fixture(
    entries: impl IntoIterator<Item = TestEntry>,
    inventory: Inventory,
    exclude: &Exclusion,
    run: impl FnOnce(&SourceContext<'_>, &mut MediaWriter<'_>, Checkpoint<'_>, &Path) -> TestResult,
) -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let archive = write_archive(&path, entries, inventory)?;
    let cancel = CancelToken::new();
    let watch = ArchiveWatch::new(&archive).with_interval(Duration::ZERO);
    let protection = ArchiveWriteProtection::new(&path, cancel.flag())?;
    let checkpoint = Checkpoint {
        cancel: &cancel,
        watch: &watch,
        protection: &protection,
    };
    let stage = StagedDirectory::create(folder.path(), ".fault-stage-", checkpoint)?;
    fs::create_dir(stage.path().join("raw"))?;
    fs::create_dir(stage.path().join("chunks"))?;
    let mut writer = MediaWriter::new(
        stage.permit(),
        vec![input_capability()],
        crate::Limits::default(),
        checkpoint,
        true,
    )?;
    let budget = InputBudget::default();
    let keys = Cell::new(0);
    let index = DirectoryIndex::new(&archive, &keys);
    let inputs = DirectoryInputs::try_new(archive.entries().len(), &budget, &writer)?;
    let (root, _) = crate::hashing::file(
        &path,
        crate::ByteScope::RootArchiveFile,
        writer.limits().root_bytes,
        &checkpoint,
    )?;
    let include = Filter::parse("*")?;
    let lookup = DirectoryLookup::new_with_inputs(
        &index,
        &root,
        &[],
        SourceSelection {
            include: &include,
            exclude,
        },
        &inputs,
    );
    let entry = archive.entries().first().ok_or("owner")?;
    let (source, payload) = super::required_input::retain(&lookup, entry, &mut writer)?;
    writer.begin(0, source.clone())?;
    let context = SourceContext {
        source: &source,
        entry,
        payload: &payload,
        directory: &lookup,
    };
    let result =
        run(&context, &mut writer, checkpoint, folder.path()).map_err(|cause| MediaError::Io {
            path: "synthetic-fault-fixture".into(),
            source: io::Error::other(cause),
        });
    drop(writer);
    stage.finish(result)?;
    Ok(())
}

fn input_capability() -> Capability {
    Capability::new(
        MediaKind::Texture,
        "synthetic.input-fault",
        "1.0.0",
        "actual synthetic input evidence",
    )
}

fn write_archive(
    path: &Path,
    entries: impl IntoIterator<Item = TestEntry>,
    inventory: Inventory,
) -> Result<Archive, Box<dyn std::error::Error + Send + Sync>> {
    const ZIP_END_RECORD_BYTES: usize = 22;
    let mut builder = ArchiveBuilder::new();
    for entry in entries {
        builder = builder.entry(entry);
    }
    let mut bytes = builder.build();
    if matches!(inventory, Inventory::Recovered) {
        bytes.truncate(
            bytes
                .len()
                .checked_sub(ZIP_END_RECORD_BYTES)
                .ok_or("end record")?,
        );
    }
    fs::write(path, bytes)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().recover(matches!(inventory, Inventory::Recovered)),
    )?)
}

pub(crate) fn lookup<'source>(
    context: &'source SourceContext<'_>,
    path: &str,
    writer: &mut MediaWriter<'_>,
) -> Result<crate::EntryInput<'source>, MediaError> {
    context
        .directory
        .lookup_input(
            crate::OriginalIdentifier::new_with_path(path),
            path,
            Some(MediaKind::Texture),
            writer,
        )?
        .input
        .ok_or_else(|| MediaError::Contract("unique synthetic input".into()))
}

pub(crate) fn has_io(error: &(dyn std::error::Error + 'static), kind: io::ErrorKind) -> bool {
    let mut current = Some(error);
    while let Some(error) = current {
        if error
            .downcast_ref::<io::Error>()
            .is_some_and(|error| error.kind() == kind)
        {
            return true;
        }
        current = error.source();
    }
    false
}

pub(crate) fn entries() -> [TestEntry; 2] {
    [
        TestEntry::stored("owner.dds", b"H"),
        TestEntry::stored("owner.dds.1", b"needed"),
    ]
}

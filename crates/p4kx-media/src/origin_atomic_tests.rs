// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, OpenOptions};
use std::io::Write as _;

use p4kx_extraction::{Archive, ArchiveWatch, ArchiveWriteProtection, ConflictPolicy};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::checkpoint::Checkpoint;
use crate::generation::Generation;
use crate::origin_read_support::{Fixture, TestResult};
use crate::origin_verify::OriginAvailability;
use crate::{ExportOptions, Index, MediaError, ProviderRegistry};
use crate::{error, export_run};

#[test]
fn corrupted_staged_field_receipt_keeps_last_complete_generation() -> TestResult {
    let fixture = Fixture::new()?;
    let committed = fs::read(fixture.output.join("media-index.json"))?;
    with_next(&fixture, |_, _, checkpoint, generation, index| {
        let receipt = index
            .field_origins
            .as_ref()
            .ok_or_else(|| MediaError::Contract("new receipt group".into()))?;
        let path = generation.stage.path().join(&receipt.fields.path);
        fs::write(&path, b"{}\n").map_err(|cause| error::io(&path, cause))?;
        assert!(
            generation
                .publish(index, ConflictPolicy::Overwrite, &checkpoint)
                .is_err(),
            "actual publication validates the new provenance artifact before replacing the index"
        );
        Ok(())
    })?;
    assert_eq!(
        fs::read(fixture.output.join("media-index.json"))?,
        committed,
        "old commit preserved"
    );
    assert_eq!(
        fixture.verify(&fixture.index)?,
        OriginAvailability::Current,
        "old receipts stay valid"
    );
    Ok(())
}

#[test]
fn cancellation_after_origin_stream_completion_preserves_last_complete_generation() -> TestResult {
    let fixture = Fixture::new()?;
    let committed = fs::read(fixture.output.join("media-index.json"))?;
    with_next(&fixture, |_, options, checkpoint, generation, index| {
        assert!(
            index.field_origins.is_some(),
            "origin streams completed before cancellation"
        );
        options.cancel.cancel();
        assert!(
            matches!(
                generation.publish(index, ConflictPolicy::Overwrite, &checkpoint),
                Err(MediaError::Cancelled)
            ),
            "actual owning cancellation survives provenance integration"
        );
        Ok(())
    })?;
    assert_eq!(
        fs::read(fixture.output.join("media-index.json"))?,
        committed,
        "old commit preserved"
    );
    assert_eq!(
        fixture.verify(&fixture.index)?,
        OriginAvailability::Current,
        "old generation remains readable"
    );
    Ok(())
}

#[test]
fn source_change_after_origin_stream_completion_preserves_last_complete_generation() -> TestResult {
    let fixture = Fixture::new()?;
    let committed = fs::read(fixture.output.join("media-index.json"))?;
    with_next(&fixture, |archive, _, checkpoint, generation, index| {
        assert!(
            index.field_origins.is_some(),
            "completed origin receipts exist"
        );
        let mut file = OpenOptions::new()
            .append(true)
            .open(archive.path())
            .map_err(|cause| error::io(archive.path(), cause))?;
        file.write_all(b"changed synthetic input")
            .map_err(|cause| error::io(archive.path(), cause))?;
        file.sync_all()
            .map_err(|cause| error::io(archive.path(), cause))?;
        assert!(
            matches!(
                generation.publish(index, ConflictPolicy::Overwrite, &checkpoint),
                Err(MediaError::Changed(_))
            ),
            "actual source watch prevents committing stale payload or provenance"
        );
        Ok(())
    })?;
    assert_eq!(
        fs::read(fixture.output.join("media-index.json"))?,
        committed,
        "old commit preserved"
    );
    assert_eq!(
        fixture.verify(&fixture.index)?,
        OriginAvailability::Current,
        "old generation remains readable"
    );
    Ok(())
}

fn with_next(
    fixture: &Fixture,
    consume: impl FnOnce(
        &Archive,
        &ExportOptions,
        Checkpoint<'_>,
        &mut Generation<'_>,
        &mut Index,
    ) -> Result<(), MediaError>,
) -> TestResult {
    let source = fixture.folder.path().join("next.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("next.bin", b"next payload"))
        .write_to(&source)?;
    let archive = Archive::open(source)?;
    let options = ExportOptions::default();
    let watch = ArchiveWatch::new(&archive);
    let protection = ArchiveWriteProtection::new(archive.path(), options.cancel.flag())?;
    let checkpoint = Checkpoint {
        cancel: &options.cancel,
        watch: &watch,
        protection: &protection,
    };
    let registry = ProviderRegistry::default();
    let providers = registry.selected(&[])?;
    let mut generation = Generation::new(&fixture.output, options.limits, &checkpoint)?;
    let result = (|| {
        let mut index = export_run::build(
            &archive,
            generation.stage.permit(),
            &options,
            &providers,
            checkpoint,
        )?;
        consume(&archive, &options, checkpoint, &mut generation, &mut index)
    })();
    generation.finish(result)?;
    Ok(())
}

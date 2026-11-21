// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{ArchiveAudio, AudioIndex, BreakReason};
use p4kx_extraction::{Archive, DataForgeEntry};

use crate::exact_audio_fixture::{self as fixture, Texts};
use crate::support::{Install, TestResult};

pub(crate) struct Original {
    pub(crate) guid: String,
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) location: String,
}

pub(crate) fn original(install: &Install, texts: &Texts) -> TestResult<Original> {
    let archive = Archive::open(install.archive())?;
    let index = AudioIndex::build(&archive);
    check_definitions(&index, texts)?;
    check_resolution(&archive, &index, texts)?;
    check_entity(&archive, &index, texts)
}

fn check_definitions(index: &AudioIndex, texts: &Texts) -> TestResult {
    assert!(
        index.issues().is_empty(),
        "the actual original index is healthy"
    );
    assert_eq!(
        index.triggers().len(),
        2,
        "both actual full trigger names were read"
    );
    for (actual, expected) in index.triggers().iter().zip([&texts.trigger, &texts.later]) {
        assert_eq!(
            actual.name(),
            expected,
            "original definition order and complete name"
        );
    }
    let first = index.trigger(&texts.trigger).ok_or("original trigger")?;
    assert_eq!(
        first.events().len(),
        texts.events.len(),
        "all original events"
    );
    for (actual, expected) in first.events().iter().zip(&texts.events) {
        assert_eq!(actual.name(), expected, "original complete event spelling");
        assert_eq!(
            actual.id(),
            p4kx_testkit::wwise_short_id(expected),
            "actual event hash"
        );
    }
    let later = index
        .trigger(&texts.later)
        .ok_or("original later trigger")?;
    assert_eq!(later.events().len(), 1, "one actual later event");
    assert_eq!(
        later.events().first().ok_or("later event")?.name(),
        texts.events.first().ok_or("original first event")?,
        "the complete later event is independently decoded"
    );
    Ok(())
}

fn check_resolution(archive: &Archive, index: &AudioIndex, texts: &Texts) -> TestResult {
    let audio = ArchiveAudio::with_index(archive, index);
    let sounds = audio
        .resolve_trigger(&texts.trigger)
        .ok_or("original resolved trigger")?;
    assert_eq!(sounds.variants().len(), 1, "one original readable sound");
    let variant = sounds.variants().first().ok_or("original variant")?;
    assert_eq!(
        variant.event().name(),
        texts.events.first().ok_or("event")?,
        "original variant event"
    );
    assert_eq!(
        variant.wem_id(),
        Some(fixture::WEM),
        "the actual embedded sound"
    );
    assert_eq!(
        variant.soundbank().path(),
        texts.bank_path(),
        "complete original soundbank path"
    );
    assert_eq!(
        sounds.breaks().len(),
        texts.events.len() - 1,
        "all original missing action links"
    );
    for (broken, expected) in sounds.breaks().iter().zip(texts.events.iter().skip(1)) {
        assert_eq!(
            broken.event().name(),
            expected,
            "the exact original broken-link event"
        );
        assert!(
            matches!(broken.reason(), BreakReason::Missing { id: 777 }),
            "the actual bank event points to missing action777"
        );
    }
    Ok(())
}

fn check_entity(archive: &Archive, index: &AudioIndex, texts: &Texts) -> TestResult<Original> {
    let loaded = DataForgeEntry::read(archive)?;
    let forge = loaded.open()?;
    let record = forge.record(0).ok_or("actual Entity Record")?;
    assert_eq!(
        record.name()?,
        format!("EntityClassDefinition.{}", texts.class),
        "original header name"
    );
    assert_eq!(
        record.tree()?.error_count(),
        0,
        "the actual custom source field decodes"
    );
    let referenced = index.entity_audio(&record)?;
    assert_eq!(
        referenced.triggers().len(),
        1,
        "one actual trigger reference"
    );
    let trigger = referenced
        .triggers()
        .first()
        .ok_or("original Entity trigger")?;
    assert_eq!(
        trigger.name(),
        texts.trigger,
        "original trigger key before MCP"
    );
    assert!(
        trigger.reference().location().ends_with(&texts.field),
        "actual long owning field coordinate"
    );
    assert!(
        trigger.reference().location().len() > 256,
        "source location really exceeds old clip"
    );
    assert_eq!(
        trigger.reference().count(),
        1,
        "one actual original occurrence"
    );
    Ok(Original {
        guid: record.guid().to_string(),
        name: record.name()?.to_owned(),
        path: record.record_path()?.to_owned(),
        location: trigger.reference().location().to_owned(),
    })
}

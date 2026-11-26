// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::Read;
use std::ops::Range;

use p4kx_archive::ObservedEntry;
use p4kx_audio::{Soundbank, Wem};
use p4kx_extraction::sound::{
    SoundOriginInput, WemOutput, WemOutputTarget, WemWrittenSizeOrigin, publish_wem_output,
};
use p4kx_extraction::{
    Archive, CancelToken, ConflictPolicy, SourceLocation, SourceUseRole, SourceUses,
};
use p4kx_testkit::TestEntry;

use super::fixture::{BANK, Fixture, PCM, TestResult, VORBIS, bank, pcm, vorbis};

fn read_only_entry(archive: &Archive) -> Result<(ObservedEntry<'_>, Vec<u8>), Box<dyn Error>> {
    let entry = archive.entries().first().ok_or("fixture Entry")?;
    let observed = archive.observed_entry(entry).ok_or("original Entry")?;
    let mut bytes = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut bytes)?;
    Ok((observed, bytes))
}

#[test]
fn a_written_wav_rebinds_size_to_the_original_archive_observation() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::zstd(PCM, pcm()).encrypted()])?;
    let (entry, bytes) = read_only_entry(&fixture.archive)?;
    let wem = Wem::parse(bytes.as_slice())?;
    let path = fixture.target.join("written.wav");
    let cancel = CancelToken::new();
    let target = WemOutputTarget::new(&path, ConflictPolicy::Overwrite, None, &cancel);
    let input = Some(SoundOriginInput::for_archive_entry(entry, None));
    let report = publish_wem_output(
        WemOutput::Wav(&wem),
        input,
        target.with_report_origins(),
        String::new,
    )?;

    assert_eq!(
        report.artifact.len,
        fs::metadata(&path)?.len(),
        "measured output length"
    );
    assert!(
        report.origin_receipt.is_some(),
        "the WAV companion was published"
    );
    let witness = report.size_origin.ok_or("written WAV size witness")?;
    check_rebinding(&fixture.archive, &witness)?;

    let reopened = Archive::open_with_key(fixture.archive.path(), Some(p4kx_testkit::test_key()))?;
    let foreign_uses = SourceUses::new();
    let foreign = witness.bind(&reopened, &foreign_uses)?;
    assert_eq!(
        foreign.contains_encrypted_source(),
        None,
        "same path and bytes do not prove identity"
    );
    assert!(
        !foreign.source_set_complete(),
        "the original occurrence is unbound"
    );
    assert!(
        foreign.source_uses_refs().is_empty(),
        "foreign evidence issued no uses"
    );
    assert_eq!(
        foreign_uses.freeze()?.iter_sources().len(),
        0,
        "foreign input was not registered"
    );

    let kept =
        WemOutputTarget::new(&path, ConflictPolicy::IfChanged, None, &cancel).with_report_origins();
    let report = publish_wem_output(WemOutput::Wav(&wem), input, kept, String::new)?;
    assert!(!report.artifact.changed, "identical output is kept");
    assert!(
        report.size_origin.is_none(),
        "an existing file receives no new origin"
    );

    let report = publish_wem_output(WemOutput::Wav(&wem), input, target, String::new)?;
    assert!(report.artifact.changed, "default replacement still writes");
    assert!(report.size_origin.is_none(), "size retention is opt-in");
    Ok(())
}

fn check_rebinding(archive: &Archive, witness: &WemWrittenSizeOrigin) -> TestResult {
    let uses = SourceUses::with_compact_use_refs();
    let field = witness.bind(archive, &uses)?;
    let inventory = uses.freeze()?;
    let selected = inventory.select_uses(field.source_uses_refs())?;
    assert_eq!(
        field.contains_encrypted_source(),
        Some(true),
        "the input is encrypted"
    );
    assert!(
        field.source_set_complete(),
        "the original WAV size inputs are complete"
    );
    assert_eq!(
        selected.iter_sources().len(),
        1,
        "only the selected physical input"
    );
    assert!(
        selected
            .iter_uses()
            .any(|input| input.role() == SourceUseRole::Conversion),
        "size includes measured conversion subjects"
    );
    assert!(selected.iter_uses().all(|input| !matches!(input.location(), SourceLocation::Metadata { field } if field == "path")), "size excludes the unrelated input name");
    Ok(())
}

#[test]
fn a_written_ogg_rebinds_the_exact_encoded_size() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::stored(VORBIS, vorbis())])?;
    let (entry, bytes) = read_only_entry(&fixture.archive)?;
    let wem = Wem::parse(bytes.as_slice())?;
    let path = fixture.target.join("written.ogg");
    let cancel = CancelToken::new();
    let target =
        WemOutputTarget::new(&path, ConflictPolicy::Overwrite, None, &cancel).with_report_origins();
    let input = Some(SoundOriginInput::for_archive_entry(entry, None));
    let report = publish_wem_output(WemOutput::Ogg(&wem), input, target, String::new)?;
    assert_eq!(
        report.artifact.len,
        fs::metadata(&path)?.len(),
        "measured Ogg length"
    );
    let uses = SourceUses::new();
    let size = report
        .size_origin
        .ok_or("written Ogg size witness")?
        .bind(&fixture.archive, &uses)?;
    assert_eq!(
        size.contains_encrypted_source(),
        Some(false),
        "Ogg size only uses the plain WEM"
    );
    assert!(size.source_set_complete(), "Ogg size inputs are complete");
    assert!(
        !size.source_uses_refs().is_empty(),
        "the exact Ogg size inputs are retained"
    );
    Ok(())
}

#[test]
fn written_prefetch_retains_size_evidence_without_a_new_companion() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::zstd(BANK, bank()).encrypted()])?;
    let (entry, bytes) = read_only_entry(&fixture.archive)?;
    let bank = Soundbank::parse(bytes.as_slice())?;
    let member = bank.wem(40).ok_or("prefetched member")?;
    assert!(
        member.is_prefetch(),
        "the input contains only the retained WEM prefix"
    );
    let path = fixture.target.join("prefetch.wem");
    let companion = fixture.target.join("prefetch.wem.p4kx-origin.jsonl");
    fs::write(&companion, b"foreign companion")?;
    let cancel = CancelToken::new();
    let target =
        WemOutputTarget::new(&path, ConflictPolicy::Overwrite, None, &cancel).with_report_origins();
    let input = Some(SoundOriginInput::for_archive_entry(
        entry,
        Some(member.source()),
    ));
    let report = publish_wem_output(WemOutput::Raw(member.bytes()), input, target, String::new)?;

    assert!(report.artifact.changed, "the prefix was committed");
    assert!(
        report.origin_receipt.is_none(),
        "the foreign companion remains in place"
    );
    assert_eq!(
        fs::read(companion)?,
        b"foreign companion",
        "the foreign bytes are preserved"
    );
    assert_eq!(
        fs::read(&path)?,
        member.bytes(),
        "the committed prefix is unchanged"
    );
    let witness = report
        .size_origin
        .ok_or("size evidence survives without a companion")?;
    check_prefetch_size(&fixture.archive, &witness, member.source().byte_range())
}

fn check_prefetch_size(
    archive: &Archive,
    witness: &WemWrittenSizeOrigin,
    range: Range<u64>,
) -> TestResult {
    let uses = SourceUses::new();
    let origin = witness.bind(archive, &uses)?;
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "the original bank is encrypted"
    );
    assert!(
        origin.source_set_complete(),
        "the prefix and selected member are observed"
    );
    let frozen = uses.freeze()?;
    let selected = frozen.select_uses(origin.source_uses_refs())?;
    assert!(
        selected.iter_uses().any(|input| {
            input.role() == SourceUseRole::Payload
                && matches!(input.location(),
            SourceLocation::Subresource { namespace, field, indices }
                if namespace == "bnkEmbeddedWem" && field == "payload"
                    && indices == &[range.start, range.end])
        }),
        "size cites the exact selected payload range"
    );
    assert!(selected.iter_uses().any(|input| {
        input.role() == SourceUseRole::Selector
            && matches!(input.location(), SourceLocation::Subresource { namespace, .. } if namespace == "bnkDidx")
    }), "size cites the bank member selector");
    Ok(())
}

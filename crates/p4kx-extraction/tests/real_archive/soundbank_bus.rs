// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;

use p4kx_extraction::bank::{BankHircBus, BankHircField, BankHircSemantic};
use p4kx_extraction::{Archive, ArchiveWatch, HircKind, Soundbank};
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(Default)]
struct Counts {
    audio: usize,
    aux: usize,
    unsupported: usize,
    malformed: usize,
    roots: usize,
    ducks: usize,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn v154_initialization_bank_bus_fields_match_their_original_words_and_spans() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let build = archive.build().ok_or("observed installed Build")?;
    let entries = archive.entries().iter().filter(|entry| {
        entry
            .path()
            .rsplit(['/', '\\'])
            .next()
            .is_some_and(|name| name.eq_ignore_ascii_case("init.bnk"))
    });
    let mut counts = Counts::default();
    for entry in entries {
        let mut bytes = Vec::new();
        archive
            .open_entry_verified(entry)?
            .read_to_end(&mut bytes)?;
        let bank = Soundbank::parse(bytes.as_slice())?;
        census(&bytes, &bank, &mut counts)?;
    }
    watch.check()?;
    assert!(
        counts.audio > 0 && counts.aux > 0,
        "both v154 bus families must be populated"
    );
    assert!(
        counts.roots > 0 && counts.ducks > 0,
        "positive conditional device and duck fields"
    );
    assert_eq!(
        counts.malformed, 0,
        "the supported source bodies consume their declared span"
    );
    report(&format!(
        "v154 bus witness in Build {}: audio {}, aux {}, roots {}, ducks {}, unsupported {}, malformed {}",
        build.change_number(),
        counts.audio,
        counts.aux,
        counts.roots,
        counts.ducks,
        counts.unsupported,
        counts.malformed,
    ));
    Ok(())
}

#[expect(
    clippy::print_stdout,
    reason = "the witness reports only aggregate original layout counts"
)]
fn report(line: &str) {
    println!("{line}");
}

fn census(bytes: &[u8], bank: &Soundbank<&[u8]>, counts: &mut Counts) -> TestResult {
    for (index, raw) in bank.objects().iter().enumerate() {
        if !matches!(raw.kind, HircKind::AudioBus | HircKind::AuxiliaryBus) {
            continue;
        }
        bank.with_hirc_source_fields(index, |header, original| -> TestResult {
            match original {
                BankHircSemantic::Bus(mut bus) => {
                    match raw.kind {
                        HircKind::AudioBus => counts.audio += 1,
                        HircKind::AuxiliaryBus => counts.aux += 1,
                        _ => return Err("the original Bus kind changed".into()),
                    }
                    if bus.format_error().is_some() {
                        counts.malformed += 1;
                    }
                    verify_original(bytes, &mut bus, counts)
                }
                BankHircSemantic::Raw(_) if header.header().version != 154 => {
                    counts.unsupported += 1;
                    Ok(())
                }
                _ => Err("a v154 Bus requires the original checked source projection".into()),
            }
        })?
        .ok_or("the exact physical HIRC occurrence must exist")??;
    }
    Ok(())
}

fn verify_original<R: Read + std::io::Seek>(
    bytes: &[u8],
    bus: &mut BankHircBus<'_, R>,
    counts: &mut Counts,
) -> TestResult {
    let original = bus.fields();
    let routing = original
        .override_bus_id
        .as_ref()
        .ok_or("readable original route")?;
    check(bytes, routing, u32::to_le_bytes)?;
    if let Some(device) = &original.device_shareset_id {
        check(bytes, device, u32::to_le_bytes)?;
        counts.roots += 1;
    }
    let settings = original
        .settings
        .channel_config_bits
        .as_ref()
        .ok_or("original Bus settings word")?;
    check(bytes, settings, u32::to_le_bytes)?;
    let declared = original
        .duck_count
        .as_ref()
        .ok_or("original duck count Word")?;
    check(bytes, declared, u32::to_le_bytes)?;
    let count = declared.value;
    let mut observed = 0;
    let rows = bus.ducks().ok_or("checked duck target rows")?;
    for row in rows {
        let row = row?;
        check(bytes, &row.bus_id, u32::to_le_bytes)?;
        check(bytes, &row.volume_bits, u32::to_le_bytes)?;
        check(bytes, &row.fade_out_time, i32::to_le_bytes)?;
        check(bytes, &row.fade_in_time, i32::to_le_bytes)?;
        check(bytes, &row.fade_curve, u8::to_le_bytes)?;
        check(bytes, &row.target_property, u8::to_le_bytes)?;
        observed += 1;
    }
    assert_eq!(
        count, observed,
        "the ordered duck list covers every original row"
    );
    counts.ducks += usize::try_from(observed)?;
    assert!(
        bus.raw_remainder().is_empty(),
        "the supported original byte span is exhausted"
    );
    Ok(())
}

fn check<T: Copy, const WIDTH: usize>(
    bytes: &[u8],
    field: &BankHircField<T>,
    encode: impl FnOnce(T) -> [u8; WIDTH],
) -> TestResult {
    let range = usize::try_from(field.range.start)?..usize::try_from(field.range.end)?;
    assert!(
        bytes.get(range) == Some(encode(field.value).as_slice()),
        "the projected scalar round trips its original bytes"
    );
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Reverse;
use std::error::Error;
use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_extraction::bank::{
    BankHircField, BankHircModulator, BankHircPlugin, BankHircSemantic, BankNodePropertyKind,
};
use p4kx_extraction::{Archive, ArchiveWatch, Entry, HircKind, HircLayout, Soundbank};
use p4kx_testkit::{real_archive_key, real_archive_path};

use crate::TestResult;

const KINDS: [HircKind; 6] = [
    HircKind::FxShareSet,
    HircKind::FxCustom,
    HircKind::Lfo,
    HircKind::Envelope,
    HircKind::AudioDevice,
    HircKind::TimeModulator,
];
const SMALL_BANK: u64 = 4 * 1024 * 1024;
const SAMPLE_GROUPS: [&[&str]; 4] = [
    &["ship", "vehicle"],
    &["weap", "projectile"],
    &["ambient", "ambience"],
    &["music"],
];

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn selected_effect_metadata_matches_the_original_v154_bytes() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let build = archive.build().ok_or("installed archive Build")?;
    let mut counts = [0; KINDS.len()];
    let entries = selected(&archive);
    for entry in &entries {
        let mut raw = Vec::new();
        archive.open_entry_verified(entry)?.read_to_end(&mut raw)?;
        let bank = Soundbank::parse(&raw)?;
        assert_eq!(bank.header().version, 154, "the original bank version");
        observe_bank(&raw, &bank, &mut counts)?;
    }
    assert!(
        counts.iter().all(|count| *count > 0),
        "each v154 kind has a positive physical witness"
    );
    watch
        .check()
        .map_err(|_| "the original archive changed during the check")?;
    report(build.change_number(), entries.len(), counts);
    Ok(())
}

fn selected(archive: &Archive) -> Vec<&Entry> {
    let mut candidates: Vec<_> = archive
        .entries()
        .iter()
        .filter(|entry| {
            entry
                .path()
                .rsplit_once('.')
                .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("bnk"))
        })
        .map(|entry| (entry, entry.path().to_ascii_lowercase()))
        .collect();
    candidates
        .sort_unstable_by_key(|(entry, _)| Reverse((entry.uncompressed_size(), entry.index())));
    let mut selected: Vec<_> = candidates.iter().take(3).map(|(entry, _)| *entry).collect();
    if let Some((entry, _)) = candidates
        .iter()
        .find(|(_, path)| path.rsplit(['/', '\\']).next() == Some("init.bnk"))
        && !selected.iter().any(|other| entry.index() == other.index())
    {
        selected.push(entry);
    }
    for terms in SAMPLE_GROUPS {
        let mut admitted = 0;
        for (entry, path) in &candidates {
            if entry.uncompressed_size() <= SMALL_BANK
                && !["speech", "localized", "localised", "voice"]
                    .iter()
                    .any(|word| path.contains(word))
                && terms.iter().any(|word| path.contains(word))
                && !selected.iter().any(|other| entry.index() == other.index())
            {
                selected.push(entry);
                admitted += 1;
                if admitted == 3 {
                    break;
                }
            }
        }
    }
    selected
}

fn observe_bank<Buffer: AsRef<[u8]>>(
    raw: &[u8],
    bank: &Soundbank<Buffer>,
    counts: &mut [usize; KINDS.len()],
) -> TestResult {
    for (index, object) in bank.objects().iter().enumerate() {
        let Some(kind) = KINDS.iter().position(|kind| object.kind == *kind) else {
            continue;
        };
        assert_eq!(
            object.layout(),
            HircLayout::Decoded,
            "exact supported physical body"
        );
        bank.with_hirc_source_fields(index, |_, source| match source {
            BankHircSemantic::Plugin(mut original) => effect(raw, &mut original),
            BankHircSemantic::Modulator(mut original) => modulator(raw, &mut original),
            _ => Err("the source-field reader classified a supported occurrence as raw".into()),
        })?
        .ok_or("physical owner")??;
        *counts.get_mut(kind).ok_or("physical kind counter")? += 1;
    }
    Ok(())
}

fn effect<R: Read + Seek>(raw: &[u8], original: &mut BankHircPlugin<'_, R>) -> TestResult {
    let body = original.object().body_offset()..original.object().body_end_exclusive();
    assert_eq!(
        original.checked_range(),
        body,
        "the admitted original range reaches the owner end"
    );
    let mut cursor = Original {
        source: raw,
        offset: body.start + 4,
    };
    let fields = &original.effect().plugin;
    cursor.field(
        fields.plugin_id.as_ref().ok_or("plugin ID")?,
        u32::from_le_bytes,
    )?;
    let parameters = fields
        .parameters
        .as_ref()
        .ok_or("declared parameter size")?;
    cursor.field(&parameters.size, u32::from_le_bytes)?;
    let end = cursor.offset + u64::from(parameters.size.value);
    assert_eq!(
        parameters.range,
        cursor.offset..end,
        "owner-relative opaque bytes"
    );
    assert!(parameters.is_complete(), "the source block is complete");
    cursor.offset = end;
    let count = fields
        .bank_data_count
        .as_ref()
        .ok_or("authored bank mapping count")?;
    cursor.field(count, u8::from_le_bytes)?;
    let authored_count = usize::from(count.value);
    let mut count = 0;
    for row in original.bank_data().ok_or("original bank mappings")? {
        let row = row?;
        cursor.field(&row.index, u8::from_le_bytes)?;
        cursor.field(&row.source_id, u32::from_le_bytes)?;
        count += 1;
    }
    assert_eq!(
        count, authored_count,
        "source order and exact mapping count"
    );
    let rtpc = original.rtpc().ok_or("original initial RTPC section")?;
    cursor.field(rtpc.declared_curve_count(), u16::from_le_bytes)?;
    verify_slots(raw, original)
}

fn verify_slots<R: Read + Seek>(raw: &[u8], original: &mut BankHircPlugin<'_, R>) -> TestResult {
    let Some(slots) = original.device_slots() else {
        return Ok(());
    };
    let count = slots.count.as_ref().ok_or("authored slot count")?;
    let mut cursor = Original {
        source: raw,
        offset: count.range.start,
    };
    cursor.field(count, u8::from_le_bytes)?;
    if count.value > 0 {
        cursor.field(
            slots
                .fields
                .bypass_all_bits
                .as_ref()
                .ok_or("conditional bypass byte")?,
            u8::from_le_bytes,
        )?;
    }
    for row in original.effect_slots().ok_or("authored device slots")? {
        let row = row?;
        cursor.field(&row.index, u8::from_le_bytes)?;
        cursor.field(
            row.effect_id.as_ref().ok_or("device effect ID")?,
            u32::from_le_bytes,
        )?;
        cursor.field(
            row.bits.as_ref().ok_or("original slot flags")?,
            u8::from_le_bytes,
        )?;
    }
    assert_eq!(
        cursor.offset,
        original.object().body_end_exclusive(),
        "device slots reach exact owner end"
    );
    Ok(())
}

fn modulator<R: Read + Seek>(raw: &[u8], original: &mut BankHircModulator<'_, R>) -> TestResult {
    let body = original.object().body_offset()..original.object().body_end_exclusive();
    assert_eq!(
        original.checked_range(),
        body,
        "the admitted original range reaches the owner end"
    );
    let mut next = body.start + 4;
    for (kind, words) in [
        (BankNodePropertyKind::Properties, 1),
        (BankNodePropertyKind::RangedModifiers, 2),
    ] {
        let rows = original
            .properties(kind)
            .ok_or("authored modulator property order")?;
        let extent = rows.extent();
        let mut cursor = Original {
            source: raw,
            offset: next,
        };
        cursor.field(&extent.declared_count, u8::from_le_bytes)?;
        let authored = u64::from(extent.declared_count.value);
        assert_eq!(
            extent.ids_range,
            cursor.offset..cursor.offset + authored,
            "the original planar ID array"
        );
        let start = cursor.offset + authored;
        next = start + authored * words * 4;
        assert_eq!(
            extent.words_range,
            Some(start..next),
            "the original planar value words"
        );
        for row in rows {
            let row = row?;
            cursor.field(&row.id, u8::from_le_bytes)?;
        }
    }
    let rtpc = original.rtpc().ok_or("original modulator RTPC")?;
    let mut cursor = Original {
        source: raw,
        offset: next,
    };
    cursor.field(rtpc.declared_curve_count(), u16::from_le_bytes)
}

struct Original<'a> {
    source: &'a [u8],
    offset: u64,
}

impl Original<'_> {
    fn field<T: PartialEq, const WIDTH: usize>(
        &mut self,
        field: &BankHircField<T>,
        decode: impl FnOnce([u8; WIDTH]) -> T,
    ) -> TestResult {
        let range = self.offset..self.offset + WIDTH as u64;
        if field.range != range || field.value != decode(self.bytes(&range)?.try_into()?) {
            return Err("an original value or exact span differs".into());
        }
        self.offset = range.end;
        Ok(())
    }

    fn bytes(&self, range: &Range<u64>) -> Result<&[u8], Box<dyn Error>> {
        self.source
            .get(usize::try_from(range.start)?..usize::try_from(range.end)?)
            .ok_or_else(|| "the original read exceeded the physical bank".into())
    }
}

#[expect(
    clippy::print_stdout,
    reason = "reports installed verification totals without identifying records"
)]
fn report(build: &str, banks: usize, counts: [usize; KINDS.len()]) {
    println!(
        "v154 effect metadata, Build {build}: banks={banks}, decoded={counts:?}, raw=0, malformed=0"
    );
}

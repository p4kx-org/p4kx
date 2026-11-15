// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Reverse;
use std::io::{Read, Seek};

use p4kx_extraction::bank::{
    BankAttenuationFields, BankHircAttenuation, BankHircField, BankHircSemantic, BankRtpcRow,
};
use p4kx_extraction::{Archive, ArchiveWatch, HircKind, HircLayout, Soundbank};
use p4kx_testkit::{real_archive_key, real_archive_path};

use crate::TestResult;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn selected_attenuation_objects_match_the_original_v154_byte_layout() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let build = archive.build().ok_or("observed installed Build")?;
    let mut entries: Vec<_> = archive
        .entries()
        .iter()
        .filter(|entry| {
            entry
                .path()
                .rsplit_once('.')
                .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("bnk"))
        })
        .collect();
    entries.sort_unstable_by_key(|entry| Reverse((entry.uncompressed_size(), entry.index())));
    let mut checked = 0;
    let mut points = 0;
    for entry in entries.into_iter().take(3) {
        let mut bytes = Vec::new();
        archive
            .open_entry_verified(entry)?
            .read_to_end(&mut bytes)?;
        let bank = Soundbank::parse(&bytes)?;
        for (index, object) in bank
            .objects()
            .iter()
            .enumerate()
            .filter(|(_, object)| object.kind == HircKind::Attenuation)
        {
            assert_eq!(
                object.layout(),
                HircLayout::Decoded,
                "selected v154 object has exact supported coverage"
            );
            points += bank
                .with_hirc_source_fields(index, |header, source| {
                    assert_eq!(header.header().version, 154, "the observed bank version");
                    let BankHircSemantic::Attenuation(attenuation) = source else {
                        return Err("original attenuation observations".into());
                    };
                    verify(&bytes, *attenuation)
                })?
                .ok_or("selected physical HIRC owner")??;
            checked += 1;
        }
    }
    assert!(
        checked > 0 && points > 0,
        "positive type-14 and conversion-point witness"
    );
    watch.check().map_err(|_| "the original Archive changed")?;
    report(build.change_number(), checked, points);
    Ok(())
}

#[expect(
    clippy::print_stdout,
    reason = "the source test reports only aggregate coverage"
)]
fn report(change: &str, objects: usize, points: usize) {
    println!(
        "v154 attenuation, Build {change}: {objects} decoded, 0 raw or malformed; {points} conversion points checked"
    );
}

type PointCount = Result<usize, Box<dyn std::error::Error>>;

fn verify<R: Read + Seek>(bank: &[u8], mut attenuation: BankHircAttenuation<'_, R>) -> PointCount {
    let object = *attenuation.object();
    assert!(
        attenuation.format_error().is_none(),
        "the complete original layout"
    );
    assert_eq!(
        attenuation.checked_range(),
        object.body_offset()..object.body_end_exclusive(),
        "the checked extent is the entire physical body"
    );
    let mut raw = Original {
        bank,
        position: usize::try_from(object.body_offset())?,
    };
    assert_eq!(
        raw.read(u32::from_le_bytes)?,
        object.id(),
        "the original object ID"
    );
    conversion_prefix(&mut raw, attenuation.fields())?;
    let points = conversion_points(&mut raw, &mut attenuation)?;
    initial_rtpc(&mut raw, &mut attenuation)?;
    assert_eq!(
        u64::try_from(raw.position)?,
        object.body_end_exclusive(),
        "the independent original cursor reaches the exact object end"
    );
    Ok(points)
}

fn conversion_prefix(raw: &mut Original<'_>, fields: &BankAttenuationFields) -> TestResult {
    raw.field(
        fields
            .height_spread_flags
            .as_ref()
            .ok_or("height-spread byte")?,
        u8::from_le_bytes,
    )?;
    let cone = fields.cone_flags.as_ref().ok_or("cone flag byte")?;
    raw.field(cone, u8::from_le_bytes)?;
    if cone.value & 1 != 0 {
        let words = fields.cone.as_ref().ok_or("enabled cone words")?;
        for field in [
            &words.inside_degrees_bits,
            &words.outside_degrees_bits,
            &words.outside_volume_bits,
            &words.low_pass_bits,
            &words.high_pass_bits,
        ] {
            raw.field(
                field.as_ref().ok_or("complete original cone word")?,
                u32::from_le_bytes,
            )?;
        }
    } else {
        assert!(fields.cone.is_none(), "disabled original cone");
    }
    assert_eq!(fields.selectors.len(), 19, "original signed selector count");
    for selector in &fields.selectors {
        raw.field(selector, i8::from_le_bytes)?;
    }
    let count = fields
        .curve_count
        .as_ref()
        .ok_or("original conversion-curve count")?;
    raw.field(count, u8::from_le_bytes)?;
    assert_eq!(
        usize::from(count.value),
        fields.curves.len(),
        "all declared physical curves"
    );
    Ok(())
}

fn conversion_points<R: Read + Seek>(
    raw: &mut Original<'_>,
    attenuation: &mut BankHircAttenuation<'_, R>,
) -> PointCount {
    let mut checked = 0;
    for ordinal in 0..attenuation.fields().curves.len() {
        let curve = attenuation
            .fields()
            .curves
            .get(ordinal)
            .ok_or("original curve header")?;
        raw.field(&curve.scaling, u8::from_le_bytes)?;
        let count = curve.point_count.as_ref().ok_or("original point count")?;
        raw.field(count, u16::from_le_bytes)?;
        let declared = usize::from(count.value);
        let mut found = 0;
        for point in attenuation.points(ordinal).ok_or("original graph points")? {
            raw.point(&point?)?;
            found += 1;
        }
        assert_eq!(
            found, declared,
            "all original conversion points are admitted"
        );
        checked += found;
    }
    Ok(checked)
}

fn initial_rtpc<R: Read + Seek>(
    raw: &mut Original<'_>,
    attenuation: &mut BankHircAttenuation<'_, R>,
) -> TestResult {
    let rows = attenuation.rtpc().ok_or("original initial RTPC count")?;
    raw.field(rows.declared_curve_count(), u16::from_le_bytes)?;
    for row in rows {
        match row? {
            BankRtpcRow::Curve { fields, .. } => {
                raw.field(&fields.rtpc_id, u32::from_le_bytes)?;
                raw.field(&fields.rtpc_type, u8::from_le_bytes)?;
                raw.field(&fields.accumulation, u8::from_le_bytes)?;
                raw.variable(&fields.parameter_id)?;
                raw.field(&fields.curve_id, u32::from_le_bytes)?;
                raw.field(&fields.scaling, u8::from_le_bytes)?;
                raw.field(&fields.point_count, u16::from_le_bytes)?;
            }
            BankRtpcRow::Point { fields, .. } => raw.point(&fields)?,
            _ => return Err("unexpected original initial RTPC row".into()),
        }
    }
    Ok(())
}

struct Original<'bank> {
    bank: &'bank [u8],
    position: usize,
}

impl Original<'_> {
    fn read<const N: usize, T>(
        &mut self,
        decode: impl FnOnce([u8; N]) -> T,
    ) -> Result<T, Box<dyn std::error::Error>> {
        let end = self
            .position
            .checked_add(N)
            .ok_or("original byte range overflow")?;
        let bytes = self
            .bank
            .get(self.position..end)
            .ok_or("original byte range")?;
        self.position = end;
        Ok(decode(bytes.try_into()?))
    }

    fn field<const N: usize, T: PartialEq>(
        &mut self,
        field: &BankHircField<T>,
        decode: impl FnOnce([u8; N]) -> T,
    ) -> TestResult {
        let start = u64::try_from(self.position)?;
        if self.read(decode)? != field.value
            || field.range != (start..u64::try_from(self.position)?)
        {
            return Err("stored value or exact byte extent differs".into());
        }
        Ok(())
    }

    fn variable(&mut self, field: &BankHircField<u32>) -> TestResult {
        let start = u64::try_from(self.position)?;
        let mut value = 0_u32;
        loop {
            let byte = self.read(u8::from_le_bytes)?;
            value = value
                .checked_mul(128)
                .ok_or("original variable parameter overflow")?
                | u32::from(byte & 0x7F);
            if byte & 0x80 == 0 {
                break;
            }
        }
        assert_eq!(value, field.value, "original variable parameter word");
        assert_eq!(
            field.range,
            start..u64::try_from(self.position)?,
            "original variable parameter extent"
        );
        Ok(())
    }

    fn point(&mut self, fields: &p4kx_extraction::bank::BankRtpcPoint) -> TestResult {
        self.field(&fields.from_bits, u32::from_le_bytes)?;
        self.field(&fields.to_bits, u32::from_le_bytes)?;
        self.field(&fields.interpolation, u32::from_le_bytes)
    }
}

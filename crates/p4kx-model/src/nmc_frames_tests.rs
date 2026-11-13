// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{NmcFrameField, NmcFrames};

const HEADER: usize = 32;
const STRIDE: usize = 208;

#[test]
fn nmc_frames_retain_original_ids_raw_words_and_filtered_helpers()
-> Result<(), Box<dyn std::error::Error>> {
    let source = fixture()?;
    let parsed = NmcFrames::read(&source)?;
    let helper = parsed.frame(0).ok_or("helper row must be retained")?;
    assert_eq!(parsed.len(), 3, "all physical records remain");
    assert_eq!(
        helper.ordinal(),
        0,
        "physical source identity is its ordered row"
    );
    assert_eq!(
        helper.stored_id(),
        1024,
        "stored ID remains distinct from source ordinal"
    );
    assert_eq!(
        helper.parent_word(),
        2,
        "forward parent occurrence is preserved"
    );
    assert_eq!(helper.geometry_type(), 3, "helper row is not filtered");
    assert_eq!(
        helper.scale_bits().first(),
        Some(&3.5_f32.to_bits()),
        "source scale stays separate"
    );
    assert_eq!(
        helper
            .local_matrix_bits()
            .first()
            .and_then(|row| row.first()),
        Some(&2.0_f32.to_bits()),
        "matrix retains exact words"
    );
    assert_eq!(
        helper
            .companion_matrix_bits()
            .first()
            .and_then(|row| row.first()),
        Some(&(-0.0_f32).to_bits()),
        "companion bits are never normalized"
    );
    assert_eq!(
        helper.range(NmcFrameField::StoredId),
        172..176,
        "original byte range"
    );
    assert!(
        NmcFrames::read(source.get(..source.len() - 1).ok_or("truncated fixture")?).is_err(),
        "a partial fixed-width row cannot be complete"
    );
    Ok(())
}

fn fixture() -> Result<[u8; HEADER + 3 * STRIDE], Box<dyn std::error::Error>> {
    let mut source = [0; HEADER + 3 * STRIDE];
    source
        .get_mut(4..8)
        .ok_or("count")?
        .copy_from_slice(&3_u32.to_le_bytes());
    let mut row = |index, within, input: &[u8]| {
        let at = HEADER + index * STRIDE + within;
        source
            .get_mut(at..at + input.len())
            .ok_or("fixture row")?
            .copy_from_slice(input);
        Ok::<(), &'static str>(())
    };
    row(0, 140, &1024_u32.to_le_bytes())?;
    row(0, 148, &2_u16.to_le_bytes())?;
    row(0, 150, &3_u16.to_le_bytes())?;
    row(0, 32, &(-0.0_f32).to_bits().to_le_bytes())?;
    row(0, 80, &2.0_f32.to_bits().to_le_bytes())?;
    row(0, 128, &3.5_f32.to_bits().to_le_bytes())?;
    row(2, 148, &u16::MAX.to_le_bytes())?;
    Ok(source)
}

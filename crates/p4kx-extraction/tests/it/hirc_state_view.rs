// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::Soundbank;
use p4kx_extraction::hirc_view::ObjectView;
use p4kx_testkit::{TestBank, TestHircObject};
use serde_json::{Value, json};

#[test]
fn object_view_state_keeps_unknown_ids_exact_bits_and_json_safe_values()
-> Result<(), Box<dyn Error>> {
    let mut authored = 3_u16.to_le_bytes().to_vec();
    let ids = [9_u16, 0xF302, 9];
    let bits = [0x3F80_0000_u32, 0x7FC1_2345, 0xFF80_0000];
    authored.extend(ids.into_iter().flat_map(u16::to_le_bytes));
    authored.extend(bits.into_iter().flat_map(u32::to_le_bytes));
    let bytes = TestBank::new(15)
        .version(145)
        .object(TestHircObject::raw(1, 12, authored.as_slice()))
        .bytes();
    let bank = Soundbank::parse(bytes.as_slice())?;
    let object = bank.objects().first().ok_or("original State object")?;
    assert_eq!(
        bank.object_body(object).get(4..),
        Some(authored.as_slice()),
        "authored bytes before the JSON projection"
    );
    let view = serde_json::to_value(ObjectView::new(0, object))?;
    assert_eq!(
        view.get("status"),
        Some(&json!("decoded")),
        "known standalone State version"
    );
    assert_eq!(
        view.pointer("/detail/property_count"),
        Some(&json!(3)),
        "authored count"
    );
    let rows = view
        .pointer("/detail/properties")
        .and_then(|value| value.as_array())
        .ok_or("original property rows")?;
    check_rows(rows, &bytes, ids, bits)?;
    Ok(())
}

fn check_rows(
    rows: &[Value],
    bytes: &[u8],
    ids: [u16; 3],
    bits: [u32; 3],
) -> Result<(), Box<dyn Error>> {
    for (row, (id, (word, expected))) in
        rows.iter().zip(ids.into_iter().zip(bits.into_iter().zip([
            json!(1.0),
            json!(null),
            json!(null),
        ])))
    {
        assert_eq!(
            row.get("id"),
            Some(&json!(id)),
            "original ID and repeated order"
        );
        assert_eq!(
            row.get("value_bits"),
            Some(&json!(word)),
            "exact source word"
        );
        assert_eq!(
            row.get("value"),
            Some(&expected),
            "JSON-safe numeric projection"
        );
        let start = row
            .pointer("/value_range/start")
            .and_then(|value| value.as_str())
            .ok_or("absolute bank-relative position")?
            .parse::<usize>()?;
        assert_eq!(
            bytes.get(start..start + size_of::<u32>()),
            Some(word.to_le_bytes().as_slice()),
            "JSON span points at original bits"
        );
    }
    Ok(())
}

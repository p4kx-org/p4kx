// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::OriginReaderLimits;

use super::{Evidence, ReadSet, invalid, read_frames};
use crate::MediaError;
use crate::origin_inventory_wire::Encryption;
use crate::origin_read_policy::{InventoryBudget, OriginReadPolicy};

const HEADER: &str = r#"{"type":"set","id":"0","members":"2"}"#;
const FIRST: &str = r#"{"type":"member","sourceUseRef":"one"}"#;
const SECOND: &str = r#"{"type":"member","sourceUseRef":"two"}"#;
const END: &str = r#"{"type":"end","id":"0"}"#;
const EOF: &str = r#"{"type":"eof","sets":"1","members":"2"}"#;

fn read(rows: &[&str]) -> Result<Vec<ReadSet>, MediaError> {
    let mut rows = rows.iter();
    let mut budget = InventoryBudget::new(OriginReadPolicy {
        line_bytes: u64::MAX,
        inventory_bytes: 4096,
        fields: OriginReaderLimits {
            fields: 1,
            metadata_bytes: 1,
            path_bytes: 1,
            field_uses: 1,
            reference_bytes: 1,
        },
    });
    read_frames(
        || {
            rows.next()
                .map(|line| serde_json::from_str(line).map_err(Into::into))
                .transpose()
        },
        &mut budget,
        |reference| {
            let encryption = match reference {
                "one" => Encryption::Unencrypted,
                "two" => Encryption::Encrypted,
                _ => return Err(invalid("unknown original source-use ID")),
            };
            Ok(Evidence {
                encryption,
                opaque: false,
            })
        },
    )
}

#[test]
fn set_reader_preserves_exact_members_and_checks_encryption() -> Result<(), MediaError> {
    let sets = read(&[HEADER, FIRST, SECOND, END, EOF])?;
    let Some(set) = sets.first() else {
        return Err(invalid("one framed set"));
    };
    assert_eq!(
        set.members,
        ["one", "two"],
        "both physical uses remain addressable"
    );
    set.evidence.validate(Some(true), true)?;
    set.evidence.validate(Some(true), false)?;
    assert!(
        set.evidence.validate(Some(false), true).is_err(),
        "the encrypted shared use cannot be marked clear"
    );
    Ok(())
}

#[test]
fn set_reader_rejects_incorrect_framing_order_and_leaf_links() {
    for rows in [
        vec![HEADER, FIRST, SECOND, END],
        vec![HEADER, FIRST, SECOND, EOF],
        vec![HEADER, SECOND, FIRST, END, EOF],
        vec![HEADER, FIRST, FIRST, END, EOF],
        vec![
            HEADER,
            FIRST,
            r#"{"type":"member","sourceUseRef":"unregistered"}"#,
            END,
            EOF,
        ],
        vec![HEADER, FIRST, SECOND, END, EOF, EOF],
    ] {
        assert!(read(&rows).is_err(), "invalid shared original use set");
    }
}

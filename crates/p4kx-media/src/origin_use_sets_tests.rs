// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_extraction::{
    InputScope, SourceInput, SourceLocation, SourceUse, SourceUseRole, SourceUses,
};
use serde::Serialize;

use super::{SetRow, Sets};
use crate::MediaError;
use crate::origin_capture::OriginCapture;
use crate::origin_contract::OriginField;
use crate::origin_contributions::OriginContributions;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(Default)]
struct Bytes(u64);

impl Write for Bytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 += bytes.len() as u64;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn capture(owner: &SourceUses, names: &[String]) -> TestResult<OriginContributions> {
    let mut capture = OriginCapture::with_limit(owner, 1 << 20);
    for name in names {
        let used = owner
            .observe(SourceInput::Unknown, InputScope::Payload)?
            .at(
                SourceLocation::Metadata {
                    field: name.clone(),
                },
                SourceUseRole::Selector,
            )?;
        capture.include_original(&OriginContributions {
            owner: owner.clone(),
            uses: [used].into(),
            shared: None,
            complete: true,
            cuts: [].into(),
        })?;
    }
    Ok(capture.finish())
}

fn write(writer: &mut Bytes, value: &impl Serialize) -> serde_json::Result<()> {
    serde_json::to_writer(&mut *writer, value)?;
    writer.0 += 1;
    Ok(())
}

#[test]
fn factored_fields_conserve_exact_uses_and_write_one_directory_membership() -> TestResult {
    const COMMON: usize = 32;
    const FIELDS: usize = 16;
    let owner = SourceUses::new();
    let names: Vec<_> = (0..COMMON).map(|index| format!("common-{index}")).collect();
    let original = capture(&owner, &names)?;
    let shared = original.clone().share_uses();
    let scope = shared.shared.as_ref().ok_or("shared directory group")?;
    let mut set_stream = Sets::default();
    let id = set_stream.register(scope)?;
    let mut flat_bytes = Bytes::default();
    let mut field_bytes = Bytes::default();
    for index in 0..FIELDS {
        let local = capture(&owner, &[format!("unique-query-{index}")])?;
        let mut capture = OriginCapture::with_limit(&owner, 1 << 20);
        capture.include_original(&shared)?;
        capture.include_original(&local)?;
        let factored = capture.finish();
        let marker = factored.field_origin()?;
        let expected = flat_union(&owner, &original, &local);
        assert_eq!(
            marker, expected,
            "field proof has exact canonical membership and markers"
        );
        assert_eq!(
            set_stream.register(scope)?,
            id,
            "repeated membership has a stable ID"
        );
        let ids: Vec<_> = factored.uses.iter().map(SourceUse::reference).collect();
        let row = OriginField::with_set(&marker, &ids, id);
        let wire = serde_json::to_value(&row)?;
        let serialized_locals = wire["sourceUsesRefs"].as_array().ok_or("local use array")?;
        assert_eq!(serialized_locals.len(), 1, "each field keeps its own query");
        assert_eq!(
            wire["sourceUseSetRef"],
            serde_json::to_value(id)?,
            "the field resolves to the original directory set"
        );
        write(&mut flat_bytes, &expected)?;
        write(&mut field_bytes, &row)?;
    }
    let mut set_bytes = Bytes::default();
    let mut serialized_members = 0;
    set_stream.visit(|row| {
        if matches!(row, SetRow::Member { .. }) {
            serialized_members += 1;
        }
        write(&mut set_bytes, &row).map_err(Into::into)
    })?;
    assert_eq!(
        serialized_members, COMMON,
        "a physical directory is written once"
    );
    assert!(
        field_bytes.0 + set_bytes.0 < flat_bytes.0,
        "both artifacts together save total output bytes"
    );
    Ok(())
}

fn flat_union(
    owner: &SourceUses,
    first: &OriginContributions,
    second: &OriginContributions,
) -> p4kx_extraction::FieldOrigin {
    let uses: Vec<_> = first
        .uses
        .iter()
        .chain(second.uses.iter())
        .cloned()
        .collect();
    owner.origin(&uses, true)
}

#[test]
fn a_cancelled_set_serializer_stops_before_following_rows() -> TestResult {
    let owner = SourceUses::new();
    let group = capture(&owner, &["first".into(), "second".into()])?.share_uses();
    let mut sets = Sets::default();
    sets.register(group.shared.as_ref().ok_or("shared group")?)?;
    let mut seen = 0;
    let failure = sets
        .visit(|row| {
            seen += 1;
            if matches!(row, SetRow::Member { .. }) {
                return Err(MediaError::Cancelled);
            }
            Ok(())
        })
        .expect_err("cancellation propagates");
    assert!(
        matches!(failure, MediaError::Cancelled),
        "cancellation keeps its original error category"
    );
    assert_eq!(seen, 2, "the remaining members and EOF were not emitted");
    Ok(())
}

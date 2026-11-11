// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Compact metadata whose billions of zero-size logical fields require an explicit materializer budget.

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType};

pub(crate) const SEGMENTS: u32 = 65_538;
pub(crate) const OWN: u16 = u16::MAX;
const STRUCT_ROW_BYTES: usize = 16;

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

pub(crate) fn database(header_bytes: usize) -> TestResult<Vec<u8>> {
    let mut shared = TestStruct::new("Shared").declared_size(0);
    for property in 0..OWN {
        shared = shared.field(format!("f{property}"), TestType::Class("Empty".to_owned()));
    }
    let mut builder = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Empty").declared_size(0))
        .structure(shared);
    for segment in 2..=SEGMENTS {
        builder = builder.structure(TestStruct::new(format!("S{segment}")).declared_size(0));
    }
    let mut bytes = builder
        .record(TestRecord::new(
            88,
            format!("S{SEGMENTS}"),
            "Many.Fields",
            "many/fields.xml",
        ))
        .build();
    for segment in 1..=SEGMENTS {
        let at = header_bytes + usize::try_from(segment)? * STRUCT_ROW_BYTES;
        patch(&mut bytes, at + 4, &(segment - 1).to_le_bytes())?;
        patch(&mut bytes, at + 8, &OWN.to_le_bytes())?;
        patch(&mut bytes, at + 10, &0_u16.to_le_bytes())?;
    }
    Ok(bytes)
}

fn patch(bytes: &mut [u8], at: usize, value: &[u8]) -> TestResult<()> {
    bytes
        .get_mut(at..at + value.len())
        .ok_or("the actual encoded source row")?
        .copy_from_slice(value);
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::fs;
use std::io::{self, Read};

use p4kx_extraction::Exclusion;
use p4kx_testkit::TestEntry;

use crate::required_input_fault_support::{self as support, Inventory, TestResult};
use crate::{MediaError, Subresource};

#[test]
fn distinct_private_inputs_can_be_nested_without_holding_a_cache_borrow() -> TestResult {
    support::with_fixture(
        [
            TestEntry::stored("owner.dds", b"H"),
            TestEntry::stored("owner.dds.1", b"first"),
            TestEntry::stored("owner.dds.2", b"second"),
        ],
        Inventory::Intact,
        &Exclusion::parse(["*.dds.*"])?,
        |context, writer, _, _| {
            let first = support::lookup(context, "owner.dds.1", writer)?;
            let second = support::lookup(context, "owner.dds.2", writer)?;
            let outer = context.with_required_input(
                &first,
                &Subresource::new("synthetic.nested", vec![0]),
                writer,
                |outer, writer| {
                    let inner = context.with_required_input(
                        &second,
                        &Subresource::new("synthetic.nested", vec![1]),
                        writer,
                        |inner, _writer| {
                            let mut bytes = Vec::new();
                            inner
                                .read_to_end(&mut bytes)
                                .map_err(|source| MediaError::Io {
                                    path: "inner".into(),
                                    source,
                                })?;
                            assert!(inner.receipt().source.raw.is_none());
                            Ok(bytes)
                        },
                    )?;
                    assert_eq!(inner, b"second");
                    let mut bytes = Vec::new();
                    outer
                        .read_to_end(&mut bytes)
                        .map_err(|source| MediaError::Io {
                            path: "outer".into(),
                            source,
                        })?;
                    assert!(outer.receipt().source.raw.is_none());
                    Ok(bytes)
                },
            )?;
            assert_eq!(outer, b"first");
            assert_eq!(
                context.directory.input_state()?.budget.sources.get(),
                3,
                "one admission per actual physical source"
            );
            for entry in fs::read_dir(&writer.root)? {
                assert!(
                    !entry?.file_name().to_string_lossy().starts_with(".entry-"),
                    "both private inputs are cleaned"
                );
            }
            Ok(())
        },
    )
}

#[test]
fn a_removed_cached_payload_fails_before_its_next_callback() -> TestResult {
    cached_damage(Damage::Removed)
}

#[test]
fn a_truncated_cached_payload_fails_before_its_next_callback() -> TestResult {
    cached_damage(Damage::Truncated)
}

#[derive(Clone, Copy)]
enum Damage {
    Removed,
    Truncated,
}

fn cached_damage(damage: Damage) -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, _, _| {
            let token = support::lookup(context, "owner.dds.1", writer)?;
            let receipt = context.with_required_input(
                &token,
                &Subresource::new("synthetic.cached", vec![0]),
                writer,
                |input, _writer| Ok(input.receipt().clone()),
            )?;
            let raw = receipt.source.raw.ok_or("retained receipt")?;
            let payload = writer.root.join(raw.path);
            if matches!(damage, Damage::Removed) {
                fs::remove_file(&payload)?;
            } else {
                fs::write(&payload, b"x")?;
            }
            let called = Cell::new(false);
            let failure = context
                .with_required_input(
                    &token,
                    &Subresource::new("synthetic.cached", vec![1]),
                    writer,
                    |_input, _writer| {
                        called.set(true);
                        Ok(())
                    },
                )
                .err()
                .ok_or("damaged cached input accepted")?;
            assert!(
                !called.get(),
                "cached input is validated before callback use"
            );
            assert!(support::has_io(
                &failure,
                if matches!(damage, Damage::Removed) {
                    io::ErrorKind::NotFound
                } else {
                    io::ErrorKind::InvalidData
                }
            ));
            assert_eq!(
                context.directory.input_state()?.budget.sources.get(),
                2,
                "failed cache reuse does not readmit a source"
            );
            Ok(())
        },
    )
}

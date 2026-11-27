// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    ArchiveEntrySource, PhysicalSource, PhysicalSourceKind, SourceEncryptionDeclaration,
    SourcedValue,
};

pub(super) fn same(left: &PhysicalSource, right: &PhysicalSource) -> bool {
    left.stored_encryption() == right.stored_encryption()
        && left.ancestors().len() == right.ancestors().len()
        && match (left.kind(), right.kind()) {
            (PhysicalSourceKind::ArchiveEntry(left), PhysicalSourceKind::ArchiveEntry(right)) => {
                entry(left, right)
            }
            _ => false,
        }
}

fn entry(left: &ArchiveEntrySource, right: &ArchiveEntrySource) -> bool {
    left.entry_index() == right.entry_index()
        && left.record_format() == right.record_format()
        && left.record_range() == right.record_range()
        && cell(left.path(), right.path())
        && cell(left.zip_flags(), right.zip_flags())
        && declarations(left, right)
}

fn cell<T: PartialEq>(left: &SourcedValue<T>, right: &SourcedValue<T>) -> bool {
    left.state() == right.state() && left.value() == right.value()
}

fn declarations(left: &ArchiveEntrySource, right: &ArchiveEntrySource) -> bool {
    let left = left.declarations();
    let right = right.declarations();
    if left.state() != right.state() {
        return false;
    }
    match (left.value(), right.value()) {
        (Some(left), Some(right)) => {
            left.len() == right.len()
                && left.iter().zip(right).all(|(left, right)| {
                    left.state() == right.state()
                        && match (left.value(), right.value()) {
                            (Some(left), Some(right)) => declaration(left, right),
                            (None, None) => true,
                            _ => false,
                        }
                })
        }
        (None, None) => true,
        _ => false,
    }
}

fn declaration(left: &SourceEncryptionDeclaration, right: &SourceEncryptionDeclaration) -> bool {
    left.kind() == right.kind()
        && left.value_offset() == right.value_offset()
        && cell(left.declared_block_length(), right.declared_block_length())
        && cell(left.raw_word(), right.raw_word())
        && cell(left.raw_bytes(), right.raw_bytes())
}

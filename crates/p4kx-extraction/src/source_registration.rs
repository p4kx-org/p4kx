// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{
    EncryptionDeclarationSource, EntryOriginStep, EntryRecordFormat, StoredEncryptionDeclaration,
};

use crate::source_file_input::FileInputRef;
use crate::source_inventory::{
    ArchiveEntrySource, ArchiveFileSource, FileSource, PhysicalSource, PhysicalSourceKind,
    PhysicalSourceRef, SourceDeclarationKind, SourceEncryptionDeclaration, SourceEncryptionState,
    SourceRecordFormat,
};
use crate::source_registry::SourceRegistry;
use crate::source_value::{SourceValue, SourcedValue};
use crate::{ArchiveByteRange, DecimalU64};

pub(crate) fn archive(
    registry: &mut SourceRegistry,
    id: PhysicalSourceRef,
    ancestors: Vec<PhysicalSourceRef>,
    step: &EntryOriginStep,
    metadata: SourceEncryptionState,
) -> PhysicalSource {
    let path = registry.metadata_value(
        &id,
        "path".to_owned(),
        SourceValue::Present(step.path().to_owned()),
        metadata,
    );
    let flags = step
        .metadata()
        .zip_header()
        .map_or(SourceValue::Absent, |header| {
            SourceValue::Present(header.general_purpose_flags())
        });
    let zip_flags = registry.metadata_value(&id, "generalPurposeFlags".to_owned(), flags, metadata);
    let declarations = declarations(registry, &id, step, metadata);
    let declarations = registry.metadata_value(
        &id,
        "encryptionDeclarations".to_owned(),
        SourceValue::Present(declarations),
        metadata,
    );
    let record_format = match step.metadata().record_format() {
        EntryRecordFormat::ZipCentralDirectory => SourceRecordFormat::ZipCentralDirectory,
        EntryRecordFormat::P4kV2 => SourceRecordFormat::P4kV2,
        _ => SourceRecordFormat::Unknown,
    };
    PhysicalSource {
        id,
        ancestors,
        stored_encryption: SourceEncryptionState::of(step.encryption()),
        binding: None,
        source: PhysicalSourceKind::ArchiveEntry(ArchiveEntrySource {
            entry_index: step.index(),
            path,
            record_format,
            record_range: ArchiveByteRange::of(step.metadata().record_range()),
            zip_flags,
            declarations,
        }),
    }
}

fn declarations(
    registry: &mut SourceRegistry,
    id: &PhysicalSourceRef,
    step: &EntryOriginStep,
    metadata: SourceEncryptionState,
) -> Vec<SourcedValue<SourceEncryptionDeclaration>> {
    step.metadata()
        .encryption_declarations()
        .iter()
        .enumerate()
        .map(|(index, value)| declaration(registry, id, index, value, metadata))
        .collect()
}

fn declaration(
    registry: &mut SourceRegistry,
    id: &PhysicalSourceRef,
    index: usize,
    declaration: &StoredEncryptionDeclaration,
    metadata: SourceEncryptionState,
) -> SourcedValue<SourceEncryptionDeclaration> {
    let path = format!("encryptionDeclarations[{index}]");
    let kind = match declaration.source() {
        EncryptionDeclarationSource::CigExtraBlock => SourceDeclarationKind::CigExtraBlock,
        EncryptionDeclarationSource::P4kV2Field => SourceDeclarationKind::P4kV2Field,
        _ => SourceDeclarationKind::Unknown,
    };
    let length = declaration
        .declared_block_len()
        .map_or(SourceValue::Absent, SourceValue::Present);
    let word = declaration
        .raw_value()
        .map_or(SourceValue::Invalid, SourceValue::Present);
    let declared_block_length =
        registry.metadata_value(id, format!("{path}.declaredBlockLength"), length, metadata);
    let raw_word = registry.metadata_value(id, format!("{path}.rawWord"), word, metadata);
    let raw_bytes = registry.metadata_value(
        id,
        format!("{path}.rawBytes"),
        SourceValue::Present(declaration.raw_value_bytes().collect()),
        metadata,
    );
    let value = SourceEncryptionDeclaration {
        kind,
        value_offset: DecimalU64::new(declaration.value_offset()),
        declared_block_length,
        raw_word,
        raw_bytes,
    };
    registry.metadata_value(id, path, SourceValue::Present(value), metadata)
}
pub(crate) fn file(
    registry: &mut SourceRegistry,
    id: PhysicalSourceRef,
    origin: FileInputRef<'_>,
) -> PhysicalSource {
    let name = registry.file_metadata_value(&id, "name".to_owned(), origin.name());
    let byte_length = registry.file_metadata_value(
        &id,
        "byteLength".to_owned(),
        SourceValue::Present(DecimalU64::new(origin.byte_length())),
    );
    PhysicalSource {
        id,
        ancestors: Vec::new(),
        stored_encryption: SourceEncryptionState::Unknown,
        binding: None,
        source: PhysicalSourceKind::File(FileSource { name, byte_length }),
    }
}

pub(crate) fn archive_file(
    registry: &mut SourceRegistry,
    id: PhysicalSourceRef,
    byte_length: u64,
) -> PhysicalSource {
    let byte_length = registry.file_metadata_value(
        &id,
        "byteLength".to_owned(),
        SourceValue::Present(DecimalU64::new(byte_length)),
    );
    PhysicalSource {
        id,
        ancestors: Vec::new(),
        stored_encryption: SourceEncryptionState::Unknown,
        binding: None,
        source: PhysicalSourceKind::ArchiveFile(ArchiveFileSource { byte_length }),
    }
}

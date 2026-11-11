// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::bank_reader::SectionReader;
use crate::hirc::{self, MediaStorage, SoundSource};
use crate::hirc_node::{self, BankNodeParentFields};
use crate::{BankError, SectionId};

const FIRST_SOUND_VERSION: u32 = 150;
pub(crate) const NODE_BASE_VERSION: u32 = 154;

/// Original scalar reads shared by Sound and `MusicTrack` media sources.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankMediaSourceFields {
    /// Original plug-in ID.
    pub plugin_id: Option<BankHircField<u32>>,
    /// Original storage byte.
    pub storage_code: Option<BankHircField<u8>>,
    /// Original media source ID.
    pub source_id: Option<BankHircField<u32>>,
    /// Original cache ID when present.
    pub cache_id: Option<BankHircField<u32>>,
    /// Original in-memory size.
    pub in_memory_size: Option<BankHircField<u32>>,
    /// Original source flags.
    pub source_bits: Option<BankHircField<u8>>,
}

#[derive(Clone, Copy)]
pub(crate) enum SourceFlags {
    Optional,
    Required,
}

/// Original scalar and range reads made while interpreting one Sound object.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankSoundFields {
    /// Original plug-in ID when readable.
    pub plugin_id: Option<BankHircField<u32>>,
    /// Original storage byte, including unknown values, when readable.
    pub storage_code: Option<BankHircField<u8>>,
    /// Original source ID when readable.
    pub source_id: Option<BankHircField<u32>>,
    /// Original cache ID when present and readable.
    pub cache_id: Option<BankHircField<u32>>,
    /// Original in-memory size when readable.
    pub in_memory_size: Option<BankHircField<u32>>,
    /// Original source flags, including reserved bits, when present.
    pub source_bits: Option<BankHircField<u8>>,
    /// Layout reached after the fixed source fields.
    pub continuation: BankSoundContinuation,
    /// Original node and plug-in fields when the node layout is selected.
    pub node: Option<BankSoundNodeFields>,
}

/// Interpretation of the bytes after a Sound's source fields.
#[derive(Debug, Default)]
#[non_exhaustive]
pub enum BankSoundContinuation {
    /// A required source field failed before the continuation was reached.
    #[default]
    Unreached,
    /// An earlier Sound layout retains the remainder as opaque bytes.
    Opaque {
        /// Exact remaining range, including an empty range.
        range: Range<u64>,
    },
    /// The body of a version with a node layout ends before a node begins.
    Absent,
    /// The observed prefix of a source plug-in block and node base.
    Node,
}

/// Original optional source plug-in block and node base fields.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankSoundNodeFields {
    /// Original source plug-in parameter length when readable.
    pub plugin_parameter_length: Option<BankHircField<u32>>,
    /// Admitted parameter bytes, including an empty range.
    pub plugin_parameters: Option<Range<u64>>,
    /// Node base reads once its position has been reached.
    pub base: Option<BankSoundNodeBase>,
}

/// Original reads and extents made while traversing the node base.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankSoundNodeBase {
    /// Original parent fields read by the node base parser.
    pub fields: BankNodeParentFields,
    /// Checked original inline State extent when its full layout was readable.
    pub state_chunk: Option<Range<u64>>,
    /// Consumed node base bytes, including its checked opaque regions.
    pub consumed: Range<u64>,
    /// Residual bytes after a complete node traversal, including an empty range.
    pub trailing: Option<Range<u64>>,
}

pub(crate) fn sound(
    version: u32,
    fields: &mut SectionReader<'_>,
) -> Result<SoundSource, BankError> {
    read(version, fields, &mut BankSoundFields::default())
}

pub(crate) fn read<F: ObjectFields>(
    version: u32,
    fields: &mut F,
    observed: &mut BankSoundFields,
) -> Result<SoundSource, F::Error> {
    let mut original = BankMediaSourceFields::default();
    let source = read_source(version, fields, &mut original, SourceFlags::Optional);
    observed.plugin_id = original.plugin_id;
    observed.storage_code = original.storage_code;
    observed.source_id = original.source_id;
    observed.cache_id = original.cache_id;
    observed.in_memory_size = original.in_memory_size;
    observed.source_bits = original.source_bits;
    let mut source = source?;
    observed.continuation = if version != NODE_BASE_VERSION {
        BankSoundContinuation::Opaque {
            range: remainder(fields),
        }
    } else if fields.remaining_bytes() == 0 {
        BankSoundContinuation::Absent
    } else {
        let mut node = BankSoundNodeFields::default();
        let parent = read_node(fields, source.is_source_plugin(), &mut node);
        observed.continuation = BankSoundContinuation::Node;
        observed.node = Some(node);
        source.parent_id = Some(parent?);
        return Ok(source);
    };
    Ok(source)
}

pub(crate) fn read_source<F: ObjectFields>(
    version: u32,
    fields: &mut F,
    observed: &mut BankMediaSourceFields,
    flags: SourceFlags,
) -> Result<SoundSource, F::Error> {
    let plugin = fields.read_u32()?;
    let plugin_id = plugin.value;
    observed.plugin_id = Some(plugin);
    let storage = fields.read_u8()?;
    let storage_value = MediaStorage::from_byte(storage.value);
    observed.storage_code = Some(storage);
    let source = fields.read_u32()?;
    let source_id = source.value;
    observed.source_id = Some(source);
    if hirc::has_cache_id(version) {
        observed.cache_id = Some(fields.read_u32()?);
    }
    let size = fields.read_u32()?;
    let in_memory_size = size.value;
    observed.in_memory_size = Some(size);
    let source_bits = if matches!(flags, SourceFlags::Required) || fields.remaining_bytes() > 0 {
        let bits = fields.read_u8()?;
        let value = bits.value;
        observed.source_bits = Some(bits);
        value
    } else {
        0
    };
    Ok(SoundSource {
        plugin_id,
        storage: storage_value,
        source_id,
        in_memory_size,
        source_bits,
        parent_id: None,
    })
}

fn read_node<F: ObjectFields>(
    fields: &mut F,
    source_plugin: bool,
    observed: &mut BankSoundNodeFields,
) -> Result<u32, F::Error> {
    if source_plugin {
        read_parameters(
            fields,
            &mut observed.plugin_parameter_length,
            &mut observed.plugin_parameters,
        )?;
    }
    let start = fields.offset();
    let mut parent_fields = BankNodeParentFields::default();
    let mut state_chunk = None;
    let parent = hirc_node::read_base_with_state(fields, &mut parent_fields, &mut state_chunk);
    observed.base = Some(BankSoundNodeBase {
        fields: parent_fields,
        state_chunk,
        consumed: start..fields.offset(),
        trailing: parent.is_ok().then(|| remainder(fields)),
    });
    let parent_id = parent?;
    if fields.remaining_bytes() > 0 {
        return Err(
            BankError::invalid(SectionId::HIRC, "bytes follow the fields of a sound").into(),
        );
    }
    Ok(parent_id)
}

pub(crate) fn read_parameters<F: ObjectFields>(
    fields: &mut F,
    parameter_length: &mut Option<BankHircField<u32>>,
    parameters: &mut Option<Range<u64>>,
) -> Result<(), F::Error> {
    let length = fields.read_u32()?;
    let size = u64::from(length.value);
    *parameter_length = Some(length);
    let start = fields.offset();
    fields.skip_bytes(size)?;
    *parameters = Some(start..fields.offset());
    Ok(())
}

fn remainder<F: ObjectFields>(fields: &F) -> Range<u64> {
    let start = fields.offset();
    start..start + fields.remaining_bytes()
}

/// Returns whether a Sound object of a bank of `version` has a layout this reader knows.
pub(crate) const fn has_sound_layout(version: u32) -> bool {
    version >= FIRST_SOUND_VERSION
}

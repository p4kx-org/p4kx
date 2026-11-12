// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;
use std::sync::{Mutex, OnceLock};

use p4kx_foundation::ByteReader;

use crate::header::Header;
use crate::issues::Issues;
use crate::record_table::RecordTable;
use crate::rows::{DataMapping, EnumDef, NO_PARENT, OptionName, PropertyDef, StructDef};
use crate::source_layout_index::SourceLayoutIndex;
use crate::strings::StringTable;
use crate::table::{Table, take};
use crate::{DataForgeError, ValueKind};

/// A DataForge database opened over its bytes.
///
/// Opening reads and checks the header and every table without copying; Records, structs and
/// enums are views that decode their rows on access.
#[derive(Debug)]
pub struct DataForge<'a> {
    pub(crate) version: u32,
    pub(crate) header_words: [u32; 2],
    pub(crate) issues: Issues,
    pub(crate) bytes: &'a [u8],
    pub(crate) structs: Table<'a, StructDef>,
    pub(crate) properties: Table<'a, PropertyDef>,
    pub(crate) enums: Table<'a, EnumDef>,
    pub(crate) mappings: Table<'a, DataMapping>,
    pub(crate) records: RecordTable<'a>,
    pub(crate) values: [&'a [u8]; ValueKind::COUNT],
    pub(crate) options: Table<'a, OptionName>,
    pub(crate) text: StringTable<'a>,
    pub(crate) blob: StringTable<'a>,
    pub(crate) instance_counts: Box<[u32]>,
    pub(crate) instance_starts: Box<[u64]>,
    pub(crate) instances: &'a [u8],
    pub(crate) source_layout_index: OnceLock<SourceLayoutIndex>,
    pub(crate) source_layout_turn: Mutex<()>,
    pub(crate) guid_index: OnceLock<Box<[u32]>>,
    pub(crate) path_index: OnceLock<Box<[u32]>>,
    pub(crate) root_index: OnceLock<Box<[(u32, u32, u32)]>>,
}

impl<'a> DataForge<'a> {
    /// Opens the DataForge stored in `bytes`, e.g. the data of the Entry `Data\Game2.dcb`.
    ///
    /// # Errors
    ///
    /// Returns [`DataForgeError::UnsupportedVersion`] for any version but 6 and 8, and another
    /// [`DataForgeError`] when a table is truncated, a row refers to a row that does not exist or
    /// the instance data does not end the input.
    pub fn open(bytes: &'a [u8]) -> Result<Self, DataForgeError> {
        let mut reader = ByteReader::new(bytes);
        let header = Header::read(&mut reader)?;
        let structs = Table::read(&mut reader, header.structs, "struct table")?;
        let properties = Table::read(&mut reader, header.properties, "property table")?;
        let enums = Table::read(&mut reader, header.enums, "enum table")?;
        let mappings = Table::read(&mut reader, header.mappings, "data mapping table")?;
        let records = RecordTable::read(&mut reader, header.records, header.record_layout)?;
        let mut values: [&[u8]; ValueKind::COUNT] = [&[]; ValueKind::COUNT];
        for kind in ValueKind::ALL {
            let count = header.values.get(kind.index()).copied().unwrap_or_default();
            let pool = take(&mut reader, count, kind.element_len(), kind.name())?;
            if let Some(slot) = values.get_mut(kind.index()) {
                *slot = pool;
            }
        }
        let options = Table::from_bytes(
            values
                .get(ValueKind::EnumOption.index())
                .copied()
                .unwrap_or_default(),
        );
        let text = take(&mut reader, header.text_len, 1, "text table")?;
        let blob = take(&mut reader, header.blob_len, 1, "blob table")?;
        let remaining = reader.remaining();
        let instances = take(&mut reader, 1, remaining, "instance data")?;
        let Blocks { counts, starts } = instance_blocks(structs, mappings, instances.len())?;
        let issues = Issues::find(properties, &records);
        let forge = Self {
            version: header.version,
            header_words: header.words,
            issues,
            bytes,
            structs,
            properties,
            enums,
            mappings,
            records,
            values,
            options,
            text: StringTable::new(text, "text"),
            blob: StringTable::new(blob, "blob"),
            instance_counts: counts,
            instance_starts: starts,
            instances,
            source_layout_index: OnceLock::new(),
            source_layout_turn: Mutex::new(()),
            guid_index: OnceLock::new(),
            path_index: OnceLock::new(),
            root_index: OnceLock::new(),
        };
        forge.check_references()?;
        Ok(forge)
    }

    /// Returns the version from the header.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Returns the number of bytes the DataForge was opened over.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.bytes.len()
    }

    pub(crate) fn byte_span(&self, bytes: &[u8]) -> Option<Range<u64>> {
        if bytes.is_empty() {
            return None;
        }
        let start = bytes
            .as_ptr()
            .addr()
            .checked_sub(self.bytes.as_ptr().addr())?;
        let end = start.checked_add(bytes.len())?;
        self.bytes.get(start..end)?;
        Some(start as u64..end as u64)
    }

    /// Returns the number of Records.
    #[must_use]
    pub const fn record_count(&self) -> usize {
        self.records.len()
    }

    /// Returns the number of struct definitions.
    #[must_use]
    pub const fn struct_count(&self) -> usize {
        self.structs.len()
    }

    /// Returns the number of property definitions over all structs.
    #[must_use]
    pub const fn property_count(&self) -> usize {
        self.properties.len()
    }

    /// Returns the number of enum definitions.
    #[must_use]
    pub const fn enum_count(&self) -> usize {
        self.enums.len()
    }

    /// Returns the number of data mappings, the blocks of struct instances.
    #[must_use]
    pub const fn data_mapping_count(&self) -> usize {
        self.mappings.len()
    }

    /// Returns the number of values in the pool of `kind`.
    #[must_use]
    pub fn value_count(&self, kind: ValueKind) -> usize {
        self.values
            .get(kind.index())
            .map_or(0, |pool| pool.len() / kind.element_len())
    }

    fn check_references(&self) -> Result<(), DataForgeError> {
        let struct_count = self.structs.len() as u64;
        for (index, def) in self.structs.iter().enumerate() {
            if def.parent != NO_PARENT && u64::from(def.parent) >= struct_count {
                return Err(out_of_range("struct", index, "struct", def.parent.into()));
            }
            let end = u64::from(def.first_property) + u64::from(def.property_count);
            if end > self.properties.len() as u64 {
                return Err(out_of_range("struct", index, "property", end - 1));
            }
        }
        for (index, def) in self.enums.iter().enumerate() {
            let end = u64::from(def.first_option) + u64::from(def.option_count);
            if end > self.options.len() as u64 {
                return Err(out_of_range("enum", index, "enum option", end - 1));
            }
        }
        for (index, def) in self.records.iter().enumerate() {
            if u64::from(def.struct_index) >= struct_count {
                return Err(out_of_range(
                    "Record",
                    index,
                    "struct",
                    def.struct_index.into(),
                ));
            }
        }
        Ok(())
    }
}

/// The number of instances of every struct and the offset of its block in the instance data.
struct Blocks {
    counts: Box<[u32]>,
    starts: Box<[u64]>,
}

/// Returns where the instances of every struct lie and checks that the instance data the data
/// mappings describe fills the `remaining` bytes exactly.
fn instance_blocks(
    structs: Table<'_, StructDef>,
    mappings: Table<'_, DataMapping>,
    remaining: usize,
) -> Result<Blocks, DataForgeError> {
    let mut counts = vec![None; structs.len()];
    let mut starts = vec![0; structs.len()];
    let mut expected = 0_u64;
    for (index, mapping) in mappings.iter().enumerate() {
        let target = usize::try_from(mapping.struct_index).ok();
        let (Some(slot), Some(def)) = (
            target.and_then(|target| counts.get_mut(target)),
            target.and_then(|target| structs.get(target)),
        ) else {
            return Err(out_of_range(
                "data mapping",
                index,
                "struct",
                mapping.struct_index.into(),
            ));
        };
        if let Some(start) = target.and_then(|target| starts.get_mut(target)) {
            *start = expected;
        }
        if slot.replace(mapping.instance_count).is_some() {
            return Err(DataForgeError::DuplicateMapping {
                struct_index: mapping.struct_index,
            });
        }
        expected = u64::from(mapping.instance_count)
            .checked_mul(u64::from(def.size))
            .and_then(|size| expected.checked_add(size))
            .ok_or(DataForgeError::TooLarge {
                part: "instance data",
            })?;
    }
    let actual = remaining as u64;
    if expected != actual {
        return Err(DataForgeError::InstanceSize { expected, actual });
    }
    Ok(Blocks {
        counts: counts.into_iter().map(Option::unwrap_or_default).collect(),
        starts: starts.into_boxed_slice(),
    })
}

pub(crate) const fn out_of_range(
    part: &'static str,
    index: usize,
    target: &'static str,
    target_index: u64,
) -> DataForgeError {
    DataForgeError::IndexOutOfRange {
        part,
        index,
        target,
        target_index,
    }
}

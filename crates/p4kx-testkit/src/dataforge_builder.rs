// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use crate::TestInstance;
use crate::bytes::{put_u16, put_u32};
use crate::dataforge_schema::{TestPool, TestProperty, TestRecord, TestStruct, TestType};
use crate::dataforge_strings::{Strings, guid_bytes, names, to_u16, to_u32};

const NONE: u32 = u32::MAX;
const TAGGED_VERSION: u32 = 8;
const ARRAY_LEN: u32 = 8;
const CONVERSION_ATTRIBUTE: u16 = 0;
const CONVERSION_COMPLEX_ARRAY: u16 = 1;
const CONVERSION_SIMPLE_ARRAY: u16 = 2;

/// Builds a synthetic DataForge database in the byte layout of version 6 or 8 from a declarative
/// description of its structs, enums and Records.
///
/// Every Record gets one instance of its struct, zero-filled except for its values and with null
/// pointers where none is set; struct values, arrays and pointers add instances and pool values.
/// Struct and enum names refer to declarations by name, and an unknown name is written as index
/// `u32::MAX`.
#[derive(Debug, Clone)]
pub struct DataForgeBuilder {
    version: u32,
    pub(crate) structs: Vec<TestStruct>,
    enums: Vec<(String, Vec<String>)>,
    pub(crate) records: Vec<TestRecord>,
    pub(crate) instances: Vec<(String, TestInstance)>,
    extra_values: [u32; TestPool::COUNT],
}

impl Default for DataForgeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DataForgeBuilder {
    /// Creates a builder for an empty version 6 database.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            version: 6,
            structs: Vec::new(),
            enums: Vec::new(),
            records: Vec::new(),
            instances: Vec::new(),
            extra_values: [0; TestPool::COUNT],
        }
    }

    /// Writes `version` into the header; version 8 uses its Record layout with the tag, every
    /// other version the layout of version 6.
    #[must_use]
    pub const fn version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    /// Adds a struct definition.
    #[must_use]
    pub fn structure(mut self, structure: TestStruct) -> Self {
        self.structs.push(structure);
        self
    }

    /// Adds an enum with its options in order.
    #[must_use]
    pub fn enumeration<S: Into<String>>(
        mut self,
        name: impl Into<String>,
        options: impl IntoIterator<Item = S>,
    ) -> Self {
        self.enums
            .push((name.into(), options.into_iter().map(Into::into).collect()));
        self
    }

    /// Adds a Record.
    #[must_use]
    pub fn record(mut self, record: TestRecord) -> Self {
        self.records.push(record);
        self
    }

    /// Adds an instance of a struct that is no Record, e.g. the target of a weak pointer;
    /// [`TestValue::Pointer`](crate::TestValue::Pointer) refers to it by `name`.
    #[must_use]
    pub fn instance(mut self, name: impl Into<String>, instance: TestInstance) -> Self {
        self.instances.push((name.into(), instance));
        self
    }

    /// Appends `count` zero-filled values to a value pool.
    #[must_use]
    pub fn values(mut self, pool: TestPool, count: u32) -> Self {
        if let Some(slot) = TestPool::STORED
            .iter()
            .position(|stored| *stored == pool)
            .and_then(|index| self.extra_values.get_mut(index))
        {
            *slot = slot.saturating_add(count);
        }
        self
    }

    /// Returns the bytes of the database.
    #[must_use]
    pub fn build(&self) -> Vec<u8> {
        Writer::new(self).finish()
    }
}

pub(crate) struct Writer<'a> {
    pub(crate) spec: &'a DataForgeBuilder,
    pub(crate) struct_index: HashMap<&'a str, u32>,
    enum_index: HashMap<&'a str, u32>,
    pub(crate) sizes: Vec<u32>,
    pub(crate) text: Strings,
    blob: Strings,
    pub(crate) names: HashMap<String, (u32, u32)>,
}

impl<'a> Writer<'a> {
    fn new(spec: &'a DataForgeBuilder) -> Self {
        let struct_index = names(spec.structs.iter().map(|s| s.name.as_str()));
        let enum_index = names(spec.enums.iter().map(|(name, _)| name.as_str()));
        let mut writer = Self {
            spec,
            struct_index,
            enum_index,
            sizes: Vec::new(),
            text: Strings::default(),
            blob: Strings::default(),
            names: HashMap::new(),
        };
        writer.sizes = (0..spec.structs.len())
            .map(|index| {
                spec.structs
                    .get(index)
                    .and_then(|structure| structure.declared_size)
                    .unwrap_or_else(|| writer.struct_size(index, spec.structs.len()))
            })
            .collect();
        writer
    }

    pub(crate) fn struct_size(&self, index: usize, depth: usize) -> u32 {
        let Some(structure) = self.spec.structs.get(index) else {
            return 0;
        };
        let Some(depth) = depth.checked_sub(1) else {
            return 0;
        };
        let inherited = structure
            .parent
            .as_deref()
            .and_then(|parent| self.struct_index.get(parent))
            .map_or(0, |parent| self.struct_size(*parent as usize, depth));
        structure
            .properties
            .iter()
            .fold(inherited, |size, property| {
                size.saturating_add(self.property_size(property, depth))
            })
    }

    pub(crate) fn property_size(&self, property: &TestProperty, depth: usize) -> u32 {
        if property.array {
            return ARRAY_LEN;
        }
        match &property.ty {
            TestType::Bool | TestType::Int8 | TestType::UInt8 => 1,
            TestType::Int16 | TestType::UInt16 => 2,
            TestType::Int32
            | TestType::UInt32
            | TestType::String
            | TestType::Single
            | TestType::Locale
            | TestType::Enum(_) => 4,
            TestType::Int64
            | TestType::UInt64
            | TestType::Double
            | TestType::StrongPointer(_)
            | TestType::WeakPointer(_) => 8,
            TestType::Guid => 16,
            TestType::Reference | TestType::ReferenceTo(_) => 20,
            TestType::Class(name) => self.struct_index.get(name.as_str()).map_or(0, |index| {
                self.spec
                    .structs
                    .get(*index as usize)
                    .and_then(|structure| structure.declared_size)
                    .unwrap_or_else(|| self.struct_size(*index as usize, depth))
            }),
        }
    }

    fn property_target(&self, ty: &TestType) -> u16 {
        let target = match ty {
            TestType::Enum(name) => self.enum_index.get(name.as_str()),
            TestType::Class(name)
            | TestType::StrongPointer(name)
            | TestType::WeakPointer(name)
            | TestType::ReferenceTo(name) => self.struct_index.get(name.as_str()),
            _ => None,
        };
        target.map_or(u16::MAX, |index| u16::try_from(*index).unwrap_or(u16::MAX))
    }

    fn finish(mut self) -> Vec<u8> {
        let spec = self.spec;
        let (structs, properties, property_count) = self.struct_tables();
        let (enums, options) = self.enum_tables();
        let instances = self.instances(options);
        let records = self.records(&instances.records);
        let mappings = Self::mappings(&instances.counts);
        let mut out = Vec::new();
        for word in [0, spec.version, 0, 0] {
            put_u32(&mut out, word);
        }
        for count in [
            to_u32(spec.structs.len()),
            property_count,
            to_u32(spec.enums.len()),
            to_u32(spec.structs.len()),
            to_u32(spec.records.len()),
        ] {
            put_u32(&mut out, count);
        }
        for pool in TestPool::HEADER {
            let own = to_u32(instances.pool(pool).len() / pool.element_len());
            put_u32(&mut out, own.saturating_add(self.extra(pool)));
        }
        put_u32(&mut out, to_u32(self.text.bytes.len()));
        put_u32(&mut out, to_u32(self.blob.bytes.len()));
        for table in [&structs, &properties, &enums, &mappings, &records] {
            out.extend_from_slice(table);
        }
        for pool in TestPool::STORED {
            out.extend_from_slice(instances.pool(pool));
            out.resize(
                out.len() + self.extra(pool) as usize * pool.element_len(),
                0,
            );
        }
        out.extend_from_slice(&self.text.bytes);
        out.extend_from_slice(&self.blob.bytes);
        for block in &instances.blocks {
            out.extend_from_slice(block);
        }
        out
    }

    fn extra(&self, pool: TestPool) -> u32 {
        TestPool::STORED
            .iter()
            .zip(self.spec.extra_values)
            .find_map(|(stored, extra)| (*stored == pool).then_some(extra))
            .unwrap_or(0)
    }

    fn struct_tables(&mut self) -> (Vec<u8>, Vec<u8>, u32) {
        let mut structs = Vec::new();
        let mut properties = Vec::new();
        let mut property_count = 0_u32;
        let sizes = self.sizes.clone();
        for (structure, size) in self.spec.structs.iter().zip(&sizes) {
            put_u32(&mut structs, self.blob.offset(&structure.name));
            put_u32(
                &mut structs,
                self.lookup_struct(structure.parent.as_deref()),
            );
            put_u16(&mut structs, to_u16(structure.properties.len()));
            put_u16(&mut structs, to_u16(property_count as usize));
            put_u32(&mut structs, *size);
            for property in &structure.properties {
                self.put_property(&mut properties, property);
                property_count = property_count.saturating_add(1);
            }
        }
        (structs, properties, property_count)
    }

    fn enum_tables(&mut self) -> (Vec<u8>, Vec<u8>) {
        let mut enums = Vec::new();
        let mut options = Vec::new();
        for (name, values) in &self.spec.enums {
            put_u32(&mut enums, self.blob.offset(name));
            put_u16(&mut enums, to_u16(values.len()));
            put_u16(&mut enums, to_u16(options.len() / 4));
            for value in values {
                put_u32(&mut options, self.blob.offset(value));
            }
        }
        (enums, options)
    }

    fn mappings(instance_counts: &[u32]) -> Vec<u8> {
        let mut mappings = Vec::new();
        for (index, count) in instance_counts.iter().enumerate() {
            put_u32(&mut mappings, *count);
            put_u32(&mut mappings, to_u32(index));
        }
        mappings
    }

    fn records(&mut self, instances: &[u32]) -> Vec<u8> {
        let mut rows = Vec::new();
        for (record, instance) in self.spec.records.iter().zip(instances) {
            let struct_index = self.lookup_struct(Some(&record.type_name));
            let instance = *instance;
            let size = self.sizes.get(struct_index as usize).copied().unwrap_or(0);
            let name = if record.without_name {
                NONE
            } else {
                self.blob.offset(&record.name)
            };
            put_u32(&mut rows, name);
            put_u32(&mut rows, self.text.offset(&record.file_name));
            if self.spec.version == TAGGED_VERSION {
                let tag = if record.without_tag {
                    NONE
                } else {
                    self.blob.offset(&record.tag)
                };
                put_u32(&mut rows, tag);
            }
            put_u32(&mut rows, struct_index);
            rows.extend_from_slice(&guid_bytes(record.guid));
            put_u16(
                &mut rows,
                record
                    .instance_index
                    .unwrap_or_else(|| to_u16(instance as usize)),
            );
            put_u16(&mut rows, to_u16(size as usize));
        }
        rows
    }

    fn put_property(&mut self, out: &mut Vec<u8>, property: &TestProperty) {
        let conversion = match (&property.array, &property.ty) {
            (false, _) => CONVERSION_ATTRIBUTE,
            (
                true,
                TestType::Class(_)
                | TestType::StrongPointer(_)
                | TestType::WeakPointer(_)
                | TestType::Reference
                | TestType::ReferenceTo(_),
            ) => CONVERSION_COMPLEX_ARRAY,
            (true, _) => CONVERSION_SIMPLE_ARRAY,
        };
        put_u32(out, self.blob.offset(&property.name));
        put_u16(out, self.property_target(&property.ty));
        put_u16(out, property.ty.code());
        put_u16(out, conversion | u16::from(property.flags) << 8);
        put_u16(out, 0);
    }

    fn lookup_struct(&self, name: Option<&str>) -> u32 {
        name.and_then(|name| self.struct_index.get(name))
            .copied()
            .unwrap_or(NONE)
    }
}

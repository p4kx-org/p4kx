// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::dataforge::out_of_range;
use crate::rows::RecordDef;
use crate::strings::{compare_ignoring_case, compare_paths, strip_xml};
use crate::{
    DataForge, DataForgeError, Guid, RecordTree, Reference, Struct, TreeBuilder, Value, ValueError,
};

/// One Record of a DataForge: its GUID, name, type and record path.
#[derive(Debug, Clone, Copy)]
pub struct Record<'f> {
    pub(crate) forge: &'f DataForge<'f>,
    pub(crate) index: u32,
    pub(crate) def: RecordDef,
}

impl<'f> Record<'f> {
    /// Returns the position of the Record in definition order, as [`DataForge::records`] lists it.
    #[must_use]
    pub const fn index(&self) -> u32 {
        self.index
    }

    /// Returns the GUID that identifies the Record.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        Guid::from_stored(self.def.guid)
    }

    /// Returns the name, e.g. `EntityClassDefinition.AEGS_Avenger`; empty when the Record has no
    /// name, which [`DataForge::issues`] reports.
    ///
    /// # Errors
    ///
    /// Returns [`DataForgeError::InvalidString`] when the name offset is not a valid string.
    pub fn name(&self) -> Result<&'f str, DataForgeError> {
        Ok(self.stored_name()?.fold(|| "", || "", |name| name))
    }

    /// Returns the record path: the file the Record is defined in, without its `.xml` extension,
    /// e.g. `libs/foundry/records/entities/spaceships/aegs_avenger`. Several Records may share one.
    ///
    /// # Errors
    ///
    /// Returns [`DataForgeError::InvalidString`] when the file name offset is not a valid string.
    pub fn record_path(&self) -> Result<&'f str, DataForgeError> {
        self.stored_file_name().map(strip_xml)
    }

    /// Returns the Record tag version 8 stores with every Record, e.g. `SystemsDesign`, or `None` for
    /// version 6, which has no tags, and for a Record without tag, which [`DataForge::issues`]
    /// reports.
    ///
    /// # Errors
    ///
    /// Returns [`DataForgeError::InvalidString`] when the tag offset is not a valid string.
    pub fn tag(&self) -> Result<Option<&'f str>, DataForgeError> {
        Ok(self.stored_tag()?.fold(|| None, || None, Some))
    }

    /// Returns the struct the Record is an instance of.
    ///
    /// # Errors
    ///
    /// Returns [`DataForgeError::IndexOutOfRange`] when the struct does not exist.
    pub fn struct_type(&self) -> Result<Struct<'f>, DataForgeError> {
        self.forge.struct_at(self.def.struct_index).ok_or_else(|| {
            out_of_range(
                "Record",
                self.index as usize,
                "struct",
                self.def.struct_index.into(),
            )
        })
    }

    /// Returns the name of the Record's struct, its type.
    ///
    /// # Errors
    ///
    /// Returns a [`DataForgeError`] when the struct or its name cannot be read.
    pub fn type_name(&self) -> Result<&'f str, DataForgeError> {
        self.struct_type()?.name()
    }

    /// Builds the canonical Record tree: type, GUID, record path, name and every field in
    /// declaration order, the inherited ones first.
    ///
    /// Inline classes and the targets of strong pointers are nested structs, arrays hold their
    /// elements in order, Reference fields and weak pointers are [`Reference`] links and null
    /// pointers are [`Value::Null`](crate::Value::Null). A value that cannot be read, such as a
    /// dangling Reference or an index out of range, becomes a
    /// [`Value::Error`](crate::Value::Error) at its place. Use a [`TreeBuilder`] to build many
    /// Records.
    ///
    /// # Errors
    ///
    /// Returns a [`DataForgeError`] when the Record's instance, the layout of a struct in it or a
    /// name cannot be read: a cycle or more than
    /// [`MAX_INHERITANCE_DEPTH`](crate::MAX_INHERITANCE_DEPTH) structs in a parent chain, fields
    /// that do not fill the declared size, an unknown data type or an inline struct that does not
    /// exist; and when the tree would hold more than
    /// [`MAX_RECORD_VALUES`](crate::MAX_RECORD_VALUES) values.
    pub fn tree(&self) -> Result<RecordTree<'f>, DataForgeError> {
        TreeBuilder::new().build(self)
    }

    /// Returns whether the Record belongs to `forge`.
    #[must_use]
    pub fn is_from(&self, forge: &DataForge<'_>) -> bool {
        std::ptr::addr_eq(self.forge, forge)
    }

    /// Returns a Reference to this Record as a value, or the error that keeps it from being one.
    pub(crate) fn link(&self) -> Value<'f> {
        self.reference().map_or_else(Value::Error, |reference| {
            Value::Reference(Box::new(reference))
        })
    }

    fn reference(&self) -> Result<Reference<'f>, ValueError> {
        let forge = self.forge;
        let guid = self.guid();
        let def = forge.structs.get(self.def.struct_index as usize);
        debug_assert!(
            def.is_some(),
            "the struct of every Record is checked on opening"
        );
        let def = def.ok_or(ValueError::DanglingReference { guid })?;
        Ok(Reference {
            record_index: self.index(),
            guid,
            record_path: forge.text.value(self.def.file_name).map(strip_xml)?,
            type_name: forge.blob.value(def.name)?,
            location: None,
        })
    }
}

impl<'a> DataForge<'a> {
    /// Returns every Record in definition order.
    pub fn records(&self) -> impl ExactSizeIterator<Item = Record<'_>> {
        self.records.iter_indexed().map(|(index, def)| Record {
            forge: self,
            index,
            def,
        })
    }

    /// Returns the Record with `guid`, or `None` when there is none.
    #[must_use]
    pub fn record_by_guid(&self, guid: Guid) -> Option<Record<'_>> {
        let index = self.guid_index.get_or_init(|| self.build_guid_index());
        let guid_of = |record: &u32| self.record_def(*record).map(|def| def.guid);
        let first = index.partition_point(|record| guid_of(record) < Some(guid.stored()));
        index
            .get(first)
            .and_then(|record| self.record(*record))
            .filter(|record| record.def.guid == guid.stored())
    }

    /// Returns the Records at a record path in definition order, ignoring ASCII case, the kind of
    /// separator and a trailing `.xml`.
    pub fn records_at_path<'s>(&'s self, path: &str) -> impl Iterator<Item = Record<'s>> + 's {
        let path = strip_xml(path);
        let index = self.path_index.get_or_init(|| self.build_path_index());
        let path_of = |record: &u32| self.path_of(*record).unwrap_or_default();
        let start = index.partition_point(|record| compare_paths(path_of(record), path).is_lt());
        let tail = index.get(start..).unwrap_or_default();
        let len = tail.partition_point(|record| compare_paths(path_of(record), path).is_eq());
        let mut found = tail.get(..len).unwrap_or_default().to_vec();
        found.sort_unstable();
        found.into_iter().filter_map(|record| self.record(record))
    }

    /// Returns the indices of the Records `text` names: the one with that GUID when `text` is a
    /// GUID, otherwise those at that record path as [`records_at_path`](Self::records_at_path)
    /// matches it; empty when no Record has it.
    #[must_use]
    pub fn records_named(&self, text: &str) -> Vec<u32> {
        text.parse::<Guid>().map_or_else(
            |_| {
                self.records_at_path(text)
                    .map(|record| record.index())
                    .collect()
            },
            |guid| {
                self.record_by_guid(guid)
                    .map(|record| record.index())
                    .into_iter()
                    .collect()
            },
        )
    }

    /// Returns the Record at `index` in definition order, the one [`Record::index`] gives, or
    /// `None` past the last Record.
    #[must_use]
    pub fn record(&self, index: u32) -> Option<Record<'_>> {
        let def = self.record_def(index)?;
        Some(Record {
            forge: self,
            index,
            def,
        })
    }

    pub(crate) fn record_def(&self, index: u32) -> Option<RecordDef> {
        self.records.get(usize::try_from(index).ok()?)
    }

    pub(crate) fn path_of(&self, index: u32) -> Option<&'a str> {
        let def = self.record_def(index)?;
        self.text.get(def.file_name).ok().map(strip_xml)
    }

    /// Returns the indices of the Records with a readable record path in Record order: by record
    /// path ignoring ASCII case with `/` and `\` alike, then by Record name ignoring ASCII case,
    /// then by definition order.
    #[must_use]
    pub fn path_order(&self) -> &[u32] {
        self.path_index.get_or_init(|| self.build_path_index())
    }

    fn build_guid_index(&self) -> Box<[u32]> {
        let mut keyed: Vec<([u8; 16], u32)> = self
            .records
            .iter_indexed()
            .map(|(index, def)| (def.guid, index))
            .collect();
        keyed.sort_unstable();
        keyed.into_iter().map(|(_, index)| index).collect()
    }

    fn build_path_index(&self) -> Box<[u32]> {
        let mut keyed: Vec<(&str, &str, u32)> = self
            .records
            .iter_indexed()
            .filter_map(|(index, def)| {
                let path = self.text.get(def.file_name).ok()?;
                let name = self.blob.get(def.name).unwrap_or_default();
                Some((strip_xml(path), name, index))
            })
            .collect();
        keyed.sort_unstable_by(|left, right| {
            compare_paths(left.0, right.0)
                .then_with(|| compare_ignoring_case(left.1, right.1))
                .then(left.2.cmp(&right.2))
        });
        keyed.into_iter().map(|(_, _, index)| index).collect()
    }
}

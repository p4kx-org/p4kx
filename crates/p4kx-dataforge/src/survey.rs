// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use rayon::prelude::*;

use crate::build::words;
use crate::pointer_offsets::PointerOffsets;
use crate::rows::NO_STRING;
use crate::{DataForge, DataForgeError, ValueKind};

const NULL: u32 = u32::MAX;

/// How the pointers of a DataForge name their target.
///
/// The instance index is a full 32-bit word: a database with more than 65535 instances of one
/// struct has pointers with a non-zero high half, so no reader may take it as 16 bits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PointerSurvey {
    /// Null pointers, which store `0xFFFFFFFF` in both words.
    pub null: u64,
    /// Pointers to an instance that exists, with an index below 65536.
    pub narrow: u64,
    /// Pointers to an instance that exists, with an index of 65536 or more.
    pub wide: u64,
    /// Pointers that are not null and name no instance.
    pub dangling: u64,
}

impl PointerSurvey {
    fn add(&mut self, forge: &DataForge<'_>, bytes: &[u8]) {
        let [struct_index, instance] = words(bytes);
        if struct_index == NULL && instance == NULL {
            self.null += 1;
        } else if forge.instance(struct_index, instance).is_none() {
            self.dangling += 1;
        } else if instance >> 16 == 0 {
            self.narrow += 1;
        } else {
            self.wide += 1;
        }
    }

    const fn merge(mut self, other: Self) -> Self {
        self.null += other.null;
        self.narrow += other.narrow;
        self.wide += other.wide;
        self.dangling += other.dangling;
        self
    }
}

/// How the last 16 bits of the Record rows relate to the size of the Record's struct.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TrailingSurvey {
    /// Records whose value equals the struct size.
    pub equals_struct_size: usize,
    /// Records whose value is zero.
    pub zero: usize,
    /// Records with any other value.
    pub other: usize,
    /// The first value that is neither the struct size nor zero.
    pub first_other: Option<u16>,
}

/// What a DataForge shows about parts of its format that no known tool interprets or that this
/// reader reads more strictly than another tool; see [`DataForge::survey`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FormatSurvey {
    /// The two header words after the version.
    pub header_words: [u32; 2],
    /// The pointers in instances and in the pointer pools.
    pub pointers: PointerSurvey,
    /// Properties with bits above the low byte of the conversion type.
    pub flagged_properties: usize,
    /// Records whose name offset is `0xFFFFFFFF`.
    pub records_without_name: usize,
    /// Records whose tag offset is `0xFFFFFFFF`.
    pub records_without_tag: usize,
    /// The last 16 bits of the Record rows.
    pub trailing: TrailingSurvey,
    /// Structs whose layout cannot be read and whose pointers are not counted.
    pub unreadable_structs: usize,
}

impl DataForge<'_> {
    /// Surveys the format details that are not interpreted: the header words, the high 16 bits of
    /// pointers, the conversion flags, Records without name or tag and the trailing 16 bits of the
    /// Record rows. It reads every instance once and runs on rayon's threads.
    ///
    /// # Errors
    ///
    /// Returns an operational source-layout failure rather than counting it as unreadable input.
    pub fn survey(&self) -> Result<FormatSurvey, DataForgeError> {
        let mut survey = FormatSurvey {
            header_words: self.header_words,
            flagged_properties: self.properties.iter().filter(|def| def.flags != 0).count(),
            ..FormatSurvey::default()
        };
        for def in self.records.iter() {
            survey.records_without_name += usize::from(def.name == NO_STRING);
            survey.records_without_tag += usize::from(def.tag == Some(NO_STRING));
            let size = self
                .structs
                .get(def.struct_index as usize)
                .map(|record| record.size);
            let trailing = &mut survey.trailing;
            if size.is_some_and(|size| size == u32::from(def.trailing)) {
                trailing.equals_struct_size += 1;
            } else if def.trailing == 0 {
                trailing.zero += 1;
            } else {
                trailing.other += 1;
                trailing.first_other.get_or_insert(def.trailing);
            }
        }
        let (pointers, unreadable) = self.survey_pointers()?;
        survey.pointers = pointers;
        survey.unreadable_structs = unreadable;
        Ok(survey)
    }

    fn survey_pointers(&self) -> Result<(PointerSurvey, usize), DataForgeError> {
        let mut pools = PointerSurvey::default();
        for kind in [ValueKind::StrongPointer, ValueKind::WeakPointer] {
            let pool = self.values.get(kind.index()).copied().unwrap_or_default();
            pool.chunks_exact(kind.element_len())
                .for_each(|bytes| pools.add(self, bytes));
        }
        let count = u32::try_from(self.structs.len()).unwrap_or(u32::MAX);
        let mut offsets = PointerOffsets::default();
        let mut unreadable = 0;
        let mut work = Vec::new();
        for struct_index in 0..count {
            if self.instance_count_of(struct_index) == 0 {
                continue;
            }
            match offsets.of(self, struct_index)? {
                Some(list) if !list.is_empty() => work.push((struct_index, list)),
                Some(_) => {}
                None => unreadable += 1,
            }
        }
        let instances = work
            .par_iter()
            .map(|(struct_index, list)| self.survey_struct(*struct_index, list))
            .reduce(PointerSurvey::default, PointerSurvey::merge);
        Ok((pools.merge(instances), unreadable))
    }

    fn instance_count_of(&self, struct_index: u32) -> u32 {
        self.instance_counts
            .get(struct_index as usize)
            .copied()
            .unwrap_or_default()
    }

    fn survey_struct(&self, struct_index: u32, offsets: &[usize]) -> PointerSurvey {
        let mut found = PointerSurvey::default();
        for instance in 0..self.instance_count_of(struct_index) {
            let bytes = self.instance(struct_index, instance).unwrap_or_default();
            for &offset in offsets {
                found.add(self, bytes.get(offset..).unwrap_or_default());
            }
        }
        found
    }
}

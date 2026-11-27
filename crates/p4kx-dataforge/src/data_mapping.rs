// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::DataForge;

/// An original physical `DataMapping` row, including an authored zero instance count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct DataMappingDeclaration {
    /// The physical `DataMapping` table index.
    pub index: u32,
    /// The declared Struct table index.
    pub struct_index: u32,
    /// The stored count of instances in the original block.
    pub instance_count: u32,
}

impl DataForge<'_> {
    /// Iterates the checked original `DataMapping` rows in physical table order.
    pub fn data_mappings(&self) -> impl ExactSizeIterator<Item = DataMappingDeclaration> + '_ {
        self.mappings
            .iter_indexed()
            .map(|(index, original)| DataMappingDeclaration {
                index,
                struct_index: original.struct_index,
                instance_count: original.instance_count,
            })
    }
}

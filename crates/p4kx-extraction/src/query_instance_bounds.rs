// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_dataforge::DataForge;

use crate::query_source_evidence::QuerySourceSite;
use crate::{SourceLocation, SourceUseRole};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Requirement {
    Count,
    Extent,
    Placement,
}

#[derive(Default)]
pub(crate) struct InstanceBounds(BTreeMap<u32, Requirement>);

impl InstanceBounds {
    pub(crate) fn count(&mut self, structure: u32) {
        self.0.entry(structure).or_insert(Requirement::Count);
    }

    pub(crate) fn extent(&mut self, structure: u32, resolved: bool) {
        let needed = if resolved {
            Requirement::Placement
        } else {
            Requirement::Extent
        };
        self.0
            .entry(structure)
            .and_modify(|previous| *previous = (*previous).max(needed))
            .or_insert(needed);
    }

    pub(crate) fn sites(mut self, forge: &DataForge<'_>) -> Option<Vec<QuerySourceSite>> {
        if self.0.is_empty() {
            return Some(Vec::new());
        }
        let mut fields = self
            .0
            .iter()
            .filter(|&(&_structure, &requirement)| requirement != Requirement::Count)
            .map(|(&structure, &_requirement)| size(structure))
            .collect::<Vec<_>>();
        let mut placements = self
            .0
            .values()
            .filter(|&&need| need == Requirement::Placement)
            .count();
        let mut prefix_end = 0;
        for original in forge.data_mappings() {
            if let Some(request) = self.0.remove(&original.struct_index) {
                fields.extend(["structIndex", "instanceCount"].map(|field| {
                    subresource(
                        "dataforgeDataMapping",
                        vec![u64::from(original.index)],
                        field,
                    )
                }));
                if request == Requirement::Placement {
                    placements -= 1;
                    prefix_end = original.index;
                }
            }
            if placements != 0 {
                fields.push(size(original.struct_index));
            }
            if self.0.is_empty() {
                break;
            }
        }
        if placements != 0 {
            return None;
        }
        if prefix_end != 0 {
            fields.extend(
                ["structIndex", "instanceCount"]
                    .map(|field| mapping_range(u64::from(prefix_end), field)),
            );
        }
        if !self.0.is_empty() {
            fields.push(QuerySourceSite::header(
                "dataMappingCount",
                SourceUseRole::Selector,
            ));
            fields.push(mapping_range(
                forge.data_mapping_count() as u64,
                "structIndex",
            ));
        }
        Some(fields)
    }
}

fn size(structure: u32) -> QuerySourceSite {
    QuerySourceSite::structure(structure as usize, "size", SourceUseRole::Selector)
}

fn mapping_range(end: u64, field: &str) -> QuerySourceSite {
    subresource("dataforgeDataMappingRange", vec![0, end], field)
}

fn subresource(namespace: &str, indices: Vec<u64>, field: &str) -> QuerySourceSite {
    QuerySourceSite {
        location: SourceLocation::Subresource {
            namespace: namespace.into(),
            indices,
            field: field.into(),
        },
        role: SourceUseRole::Selector,
    }
}

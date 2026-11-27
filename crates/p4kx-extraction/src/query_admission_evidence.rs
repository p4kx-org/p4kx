// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Original Record path and type admission sites for an observed DataForge Query.

use std::collections::BTreeMap;
use std::ops::Range;

use p4kx_dataforge::query::{QueryAdmissionOutcome, QueryRecordAdmission, QueryTypeSource};

use crate::dataforge_layout_evidence;
use crate::query_source_evidence::QuerySourceSite;
use crate::source_use_ref_encoding::RECORD_PATH_TEXT_RANGES;
use crate::{RecordOrderingName, RecordPathObservation, SourceLocation, SourceUseRole};

#[derive(Default)]
pub(crate) struct HeaderCensus {
    file_names: Intervals,
    record_names: Intervals,
    record_types: Intervals,
    text: Intervals,
}

impl HeaderCensus {
    pub(crate) fn record(&mut self, observation: &QueryRecordAdmission) -> Option<QuerySourceSite> {
        if observation.structure_index.is_some() {
            self.record_types.record(observation.record_index);
        }
        self.path(
            observation.record_index,
            observation.outcome != QueryAdmissionOutcome::UnreadablePath,
            observation.file_name.as_ref(),
            observation.record_name.as_ref(),
            observation.name_state,
        )
    }

    pub(crate) fn all_records(
        &mut self,
        observation: &RecordPathObservation,
    ) -> Option<QuerySourceSite> {
        self.path(
            observation.record_index,
            observation.indexed,
            observation.file_name.as_ref(),
            observation.record_name.as_ref(),
            observation.name_state,
        )
    }

    fn path(
        &mut self,
        record: u32,
        indexed: bool,
        filename: Option<&Range<u64>>,
        name: Option<&Range<u64>>,
        name_state: RecordOrderingName,
    ) -> Option<QuerySourceSite> {
        if !indexed {
            return Some(QuerySourceSite::record(
                record,
                "fileName",
                SourceUseRole::Attempted,
            ));
        }

        self.file_names.record(record);
        for span in [filename, name].into_iter().flatten() {
            self.text.insert(span.clone());
        }

        if name_state == RecordOrderingName::Unreadable {
            return Some(QuerySourceSite::record(
                record,
                "recordName",
                SourceUseRole::Attempted,
            ));
        }
        self.record_names.record(record);
        None
    }

    pub(crate) fn sites(self) -> impl Iterator<Item = QuerySourceSite> {
        self.file_names
            .record_sites("fileName")
            .chain(self.record_names.record_sites("recordName"))
            .chain(self.record_types.record_sites("sourceType"))
            .chain(self.text.path_text_site())
    }
}

#[derive(Default)]
struct Intervals(BTreeMap<u64, u64>);

impl Intervals {
    fn record(&mut self, position: u32) {
        let start = u64::from(position);
        self.insert(start..start + 1);
    }

    fn insert(&mut self, mut range: Range<u64>) {
        if range.is_empty() {
            return;
        }
        if let Some((&start, &end)) = self.0.range(..=range.start).next_back()
            && end >= range.start
        {
            self.0.remove(&start);
            range.start = start;
            range.end = range.end.max(end);
        }
        while let Some((&start, &end)) = self.0.range(range.start..=range.end).next() {
            self.0.remove(&start);
            range.end = range.end.max(end);
        }
        self.0.insert(range.start, range.end);
    }

    fn path_text_site(self) -> Option<QuerySourceSite> {
        if self.0.is_empty() {
            return None;
        }

        let indices = self.0.into_iter().flat_map(<[u64; 2]>::from).collect();
        Some(QuerySourceSite {
            location: SourceLocation::Subresource {
                namespace: RECORD_PATH_TEXT_RANGES.into(),
                indices,
                field: "text".into(),
            },
            role: SourceUseRole::Selector,
        })
    }

    fn record_sites(self, field: &'static str) -> impl Iterator<Item = QuerySourceSite> {
        self.0.into_iter().map(move |(start, end)| QuerySourceSite {
            location: SourceLocation::Subresource {
                namespace: "dataforgeRecordHeaderRange".into(),
                indices: vec![start, end],
                field: field.into(),
            },
            role: SourceUseRole::Selector,
        })
    }
}

/// Addresses one actual type-name or parent word used for FROM.
#[must_use]
pub fn type_sites(candidate: &QueryTypeSource) -> Vec<QuerySourceSite> {
    match candidate {
        QueryTypeSource::Name { index, span, .. } => {
            dataforge_layout_evidence::type_name(*index, span.as_ref())
                .into_iter()
                .flatten()
                .collect()
        }
        QueryTypeSource::Parent { index, .. } => vec![QuerySourceSite::structure(
            *index,
            "parent",
            SourceUseRole::Selector,
        )],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::ops::Range;

    use crate::{InputScope, SourceInput, SourceLocation, SourceUse, SourceUses};

    use super::{Intervals, RECORD_PATH_TEXT_RANGES};

    #[test]
    fn merges_only_observed_bytes_in_any_order() {
        let mut ranges = Intervals::default();
        for observed in [12..15, 2..4, 9..12, 0..2, 10..13, 18..23, 19..20, 5..5] {
            ranges.insert(observed);
        }
        assert_eq!(
            ranges.0.into_iter().collect::<Vec<_>>(),
            [(0, 4), (9, 15), (18, 23)],
            "between source ranges no byte was observed"
        );
    }

    #[test]
    fn path_text_group_uses_a_bounded_registry_reference() -> Result<(), Box<dyn Error>> {
        let sources = SourceUses::with_compact_coordinates();
        let scope = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
        let spans = (0..1024).map(|i| i * 3..i * 3 + 1);
        let bind = |ranges: Intervals| -> Result<SourceUse, Box<dyn Error>> {
            Ok(ranges
                .path_text_site()
                .ok_or("observed ranges")?
                .bind(&scope)?)
        };
        let accumulate = |spans: &mut dyn Iterator<Item = Range<u64>>| {
            let mut ranges = Intervals::default();
            for span in spans {
                ranges.insert(span);
            }
            ranges
        };

        let large = bind(accumulate(&mut spans.clone()))?;
        let reordered = bind(accumulate(&mut spans.clone().rev()))?;
        let small = bind(accumulate(&mut std::iter::once(0..1)))?;
        assert_eq!(
            large.reference(),
            reordered.reference(),
            "order is canonical"
        );
        assert_ne!(
            large.reference(),
            small.reference(),
            "different sites stay distinct"
        );
        assert_eq!(
            large.reference().as_str().len(),
            small.reference().as_str().len(),
            "a reference does not repeat the coordinates"
        );

        let inventory = sources.freeze()?;
        assert_eq!(
            inventory.use_count(),
            2,
            "both uses are registered exactly once"
        );
        let site = inventory
            .iter_uses()
            .find(|site| site.id() == large.reference())
            .ok_or("the original path text use resolves")?;
        let SourceLocation::Subresource {
            namespace,
            indices,
            field,
        } = site.location()
        else {
            return Err("the original coordinate uses the structured locator".into());
        };
        assert_eq!(namespace, RECORD_PATH_TEXT_RANGES, "decoded byte domain");
        assert_eq!(field, "text", "the locator identifies text");
        let expected: Vec<_> = spans.flat_map(|span| [span.start, span.end]).collect();
        assert_eq!(indices, &expected, "the locator never encloses unread gaps");
        Ok(())
    }
}

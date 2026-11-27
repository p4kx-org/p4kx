// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use super::ReferenceIndex;
use super::collect::Chunk;
use super::observed::{Evidence, EvidencePolicy};

/// Joins the chunks, which arrive in Record order, into one index of at most `limit` References.
/// The caller drops each chunk once it is pushed, so only one wave of chunks lives at a time.
pub(super) struct Assembler {
    index: ReferenceIndex,
    shape_ids: HashMap<Box<str>, usize>,
    limit: usize,
    evidence: Option<Evidence>,
}

impl Assembler {
    pub(super) fn new(limit: usize, observed: bool, policy: EvidencePolicy) -> Self {
        Self {
            index: ReferenceIndex::default(),
            shape_ids: HashMap::new(),
            limit,
            evidence: observed.then(|| Evidence::new(policy)),
        }
    }

    /// Returns whether References were left out, so that no later chunk can add any.
    pub(super) fn is_full(&self) -> bool {
        self.index.truncated || self.evidence.as_ref().is_some_and(Evidence::aborts_index)
    }

    /// Appends the References of `chunk`, those past the limit are dropped and mark the index as
    /// truncated.
    pub(super) fn push(&mut self, mut chunk: Chunk) {
        if self.is_full() {
            return;
        }
        self.index.unreadable += chunk.unreadable;
        let room = self.limit.saturating_sub(self.index.sources.len());
        let take = chunk.sources.len().min(room);
        if take < chunk.sources.len() || chunk.truncated {
            self.index.truncated = true;
        }
        if let (Some(evidence), Some(original)) = (&mut self.evidence, chunk.evidence.take())
            && let Err(error) = evidence.append(original, take)
        {
            evidence.fail(error);
            if evidence.aborts_index() {
                return;
            }
        }
        if take > 0 {
            self.append_columns(&chunk, take);
        }
    }

    fn append_columns(&mut self, chunk: &Chunk, take: usize) {
        let base = self.index.sources.len();
        let remap = remap_shapes(&mut self.index, &mut self.shape_ids, chunk);
        let index_base = self.index.indices.len();
        let indices_end = chunk
            .index_starts
            .get(take)
            .map_or(chunk.indices.len(), |end| *end);
        let index = &mut self.index;
        index.sources.extend_from_slice(head(&chunk.sources, take));
        index.targets.extend_from_slice(head(&chunk.targets, take));
        index.shapes.extend(
            head(&chunk.shapes, take)
                .iter()
                .filter_map(|local| remap.get(*local)),
        );
        let shifted = head(&chunk.index_starts, take)
            .iter()
            .map(|start| start + index_base);
        index.index_starts.extend(shifted);
        index
            .indices
            .extend_from_slice(chunk.indices.get(..indices_end).unwrap_or_default());
        for (local, guid) in &chunk.unresolved {
            if *local < take {
                index.unresolved.push((base + *local, *guid));
            }
        }
    }

    /// Builds the lookup tables of the index over `record_count` Records.
    pub(super) fn finish(mut self, record_count: usize) -> (ReferenceIndex, Option<Evidence>) {
        if self
            .evidence
            .as_ref()
            .is_none_or(|evidence| !evidence.aborts_index())
        {
            self.index.finish(record_count);
        }
        (self.index, self.evidence)
    }
}

fn head<T>(items: &[T], count: usize) -> &[T] {
    items.get(..count).unwrap_or_default()
}

/// Adds the shapes of `chunk` the index lacks and returns the index id of each local shape.
fn remap_shapes(
    index: &mut ReferenceIndex,
    known: &mut HashMap<Box<str>, usize>,
    chunk: &Chunk,
) -> Vec<usize> {
    let mut local: Vec<(&str, usize)> = chunk
        .shape_ids
        .iter()
        .map(|(text, id)| (&**text, *id))
        .collect();
    local.sort_unstable_by_key(|(_, id)| *id);
    local
        .into_iter()
        .map(|(text, _)| {
            if let Some(id) = known.get(text) {
                return *id;
            }
            let id = index.shape_ends.len();
            index.shape_text.push_str(text);
            index.shape_ends.push(index.shape_text.len());
            known.insert(text.into(), id);
            id
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Assembler, Chunk, Evidence, EvidencePolicy};
    use crate::ReferenceIndexError;

    fn one(source: u32, target: u32, evidence: Evidence) -> Chunk {
        Chunk {
            sources: vec![source],
            targets: vec![target],
            shapes: vec![0],
            index_starts: vec![0],
            shape_ids: [("link".into(), 0)].into(),
            evidence: Some(evidence),
            ..Chunk::default()
        }
    }

    #[test]
    fn optional_evidence_failure_does_not_stop_indexing() {
        let policy = EvidencePolicy::Optional;
        let mut original = Evidence::new(policy);
        original.fail(ReferenceIndexError::Inconsistent);
        let mut assembly = Assembler::new(usize::MAX, true, policy);
        assembly.push(one(0, 2, original));
        assembly.push(one(1, 2, Evidence::new(policy)));
        let (index, evidence) = assembly.finish(3);
        let sources: Vec<_> = index.incoming(2).map(|link| link.source()).collect();

        assert_eq!(
            sources,
            [0, 1],
            "the canonical and following chunk remain indexed"
        );
        assert!(
            evidence.is_some_and(|original| original.failed()),
            "failed proof cannot be published"
        );
    }
}

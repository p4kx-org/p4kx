// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ops::ControlFlow;

use p4kx_extraction::SourceLocation;

use super::{CONFIG, entity, open, root_of, socpak, summary, written};
use crate::catalog_read::original::with_entry_capture;
use crate::loot_entxml::documents::LootEntityContainerDocument;
use crate::loot_walk::{self, DocumentVisit};
use crate::starmap_index::Index;
use crate::{CatalogError, ExportAccountingError};

type DocumentCallback = fn(usize, LootEntityContainerDocument) -> Result<(), CatalogError>;

#[test]
fn census_includes_checked_empty_links_and_cannot_close_over_failed_documents() {
    for (second, complete) in [("<Entity/>", true), ("<Entity", false)] {
        let directory = tempfile::tempdir().expect("fixture directory");
        let archive = open(
            directory.path(),
            vec![
                socpak(
                    "a",
                    &["<Entity/>".into(), entity(&written("b"), Some(CONFIG))],
                ),
                socpak("b", &[second.into()]),
            ],
        );
        let root = root_of(&archive, "a");
        let index = Index::new(&archive);
        with_entry_capture(&archive, |capture| {
            let walked = loot_walk::visit_documents(
                &archive,
                &index,
                std::iter::once(root.entry),
                None,
                capture,
                None::<DocumentVisit<DocumentCallback>>,
            )?;
            let census = walked.document_census
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            assert_eq!(walked.summaries.len(), 2, "both seeded and reached owners are visited");
            assert_eq!(census.source_set_complete(), complete, "failed EntXML blocks a definitive search");
            let inventory = capture.source_uses().finish_with(|| Ok::<(), CatalogError>(()))?;
            let documents = inventory.iter_uses().filter(|used| {
                census.source_uses_refs().contains(used.id())
                    && matches!(used.location(), SourceLocation::Subresource { namespace, field, .. }
                        if namespace == "archiveEntryRead" && field == "content")
            }).count();
            assert_eq!(documents, 2 + usize::from(complete), "parsed empty links prove no matching declaration");
            Ok(())
        }).expect("original EntXML census");
    }
}

#[test]
fn a_scoped_leaf_stops_at_the_same_physical_occurrence_as_the_writer() {
    let summaries = HashMap::from([
        (0, summary(&[1, 2], &["root"])),
        (1, summary(&[], &["match", "unvisited"])),
        (2, summary(&[0], &["match"])),
    ]);
    let canonical = loot_walk::below(0, &summaries);
    let preferred = canonical
        .iter()
        .find(|placed| placed.path == "match")
        .expect("one selected occurrence");
    let mut visited = Vec::new();
    let selected = loot_walk::visit_below(0, &summaries, |placed| {
        visited.push(placed.path.as_str());
        if placed.path == "match" {
            ControlFlow::Break(placed)
        } else {
            ControlFlow::Continue(())
        }
    })
    .break_value()
    .expect("one scoped occurrence");
    assert_eq!(
        visited,
        ["root", "match"],
        "the scoped lookup stops at the first match"
    );
    assert!(
        std::ptr::eq(*preferred, selected),
        "both retain the same physical occurrence"
    );
    assert!(
        std::ptr::eq(
            std::ptr::from_ref(&summaries[&2].placed[0]),
            std::ptr::from_ref(selected)
        ),
        "retain the writer's established LIFO order"
    );
}

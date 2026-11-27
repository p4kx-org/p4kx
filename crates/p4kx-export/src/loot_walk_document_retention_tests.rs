// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::super::container::{self, ContainerRead, DocumentOutput};
use super::{CONFIG, entity, open, root_of, socpak, written};
use crate::catalog_read::original::with_entry_capture;
use crate::starmap_index::Index;

#[test]
fn discarding_intermediate_document_rows_preserves_link_and_read_accounting()
-> Result<(), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let archive = open(
        folder.path(),
        vec![
            socpak(
                "source",
                &[
                    String::from("not XML"),
                    entity(&written("target"), Some(CONFIG)),
                ],
            ),
            socpak("target", &[]),
        ],
    );
    let index = Index::new(&archive);
    let source = root_of(&archive, "source").entry;
    let target = root_of(&archive, "target").entry;
    with_entry_capture(&archive, |capture| {
        let directory = capture
            .source_uses()
            .observe_root_directory_paths(&archive)?;
        let read = |output| {
            container::summarise(
                &archive,
                &index,
                source,
                None,
                capture,
                directory.as_ref(),
                output,
            )
        };
        let documents = read(DocumentOutput::Keep(None))?;
        let routes = read(DocumentOutput::LinksOnly)?;
        assert_same_source_accounting(&documents, &routes, target);
        Ok(())
    })?;
    Ok(())
}

fn assert_same_source_accounting(documents: &ContainerRead, routes: &ContainerRead, target: usize) {
    assert_eq!(
        documents.documents.len(),
        1,
        "the valid file produces a document row"
    );
    assert!(
        routes.documents.is_empty(),
        "an intermediate container retains no document rows"
    );
    assert_eq!(
        routes.summary, documents.summary,
        "physical reach and original link origins"
    );
    assert_eq!(
        routes.summary.nested,
        [target],
        "the valid target follows the canonical route"
    );
    assert_eq!(
        routes.ledger.entries, documents.ledger.entries,
        "the same actual read outcomes"
    );
    assert!(
        documents.scope_unknown && routes.scope_unknown,
        "the malformed file still blocks completeness"
    );
    assert_eq!(
        routes.route_unknown, documents.route_unknown,
        "identical link coverage"
    );
}

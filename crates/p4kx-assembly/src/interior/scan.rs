// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{Archive, EntryOrigin, PathKeys};
use p4kx_model::{ChildContainer, IncludedObjects, ObjectContainer, SocEntities, SocEntitiesError};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use super::files::{FileFailure, find_first, is_plain, path_key, read_file};
use super::tile_light::{self, TileDescriptor};
use crate::{AssemblyError, IssueKind};

pub(super) struct StreamScan {
    pub(super) name: String,
    pub(super) objects: Option<IncludedObjects>,
    pub(super) entities: Option<SocEntities>,
    pub(super) origin: Option<Arc<EntryOrigin>>,
}

pub(super) struct Scan {
    pub(super) children: Vec<ChildContainer>,
    pub(super) streams: Vec<StreamScan>,
    pub(super) issues: Vec<IssueKind>,
    pub(super) fatal: Option<AssemblyError>,
    pub(super) descriptor: Option<TileDescriptor>,
}

pub(super) fn scan_container(
    archive: &Archive,
    keys: &PathKeys,
    stem: &str,
    capture_tile: bool,
) -> Scan {
    let index = keys.index(archive);
    let mut scan = Scan {
        children: Vec::new(),
        streams: Vec::new(),
        issues: Vec::new(),
        fatal: None,
        descriptor: None,
    };
    let candidates = [format!("{stem}.xml"), format!("{stem}\\{stem}.xml")];
    let mut names: Vec<String> = Vec::new();
    match find_first(&index, &candidates) {
        None => scan.issues.push(IssueKind::FileMissing {
            file: candidates[0].clone(),
        }),
        Some(entry) => match read_file(archive, entry) {
            Err(FileFailure::Fatal(error)) => scan.fatal = Some(AssemblyError::Provider(error)),
            Err(failure) => scan.issues.push(IssueKind::FileUnreadable {
                file: entry.path().to_owned(),
                reason: failure.reason(),
            }),
            Ok(bytes) => {
                match ObjectContainer::parse(&bytes) {
                    Ok(container) => {
                        names = container.streams;
                        scan.children = container.children;
                    }
                    Err(error) => scan.issues.push(IssueKind::FileUnreadable {
                        file: entry.path().to_owned(),
                        reason: describe(&error),
                    }),
                }
                if capture_tile && tile_light::unique_descriptor(&index, &candidates) {
                    scan.descriptor = Some(TileDescriptor {
                        origin: archive.entry_origin(entry).map(Arc::new),
                        name: entry.path().to_owned(),
                        bytes,
                    });
                }
            }
        },
    }
    if names.is_empty() && scan.children.is_empty() && index.find(&format!("{stem}.soc")).is_some()
    {
        names.push(format!("{stem}.soc"));
    }
    let mut seen = std::collections::HashSet::new();
    names.retain(|name| seen.insert(path_key(name)));
    let read: Vec<(StreamScan, Vec<IssueKind>, Option<AssemblyError>)> = names
        .par_iter()
        .map(|name| read_stream(archive, &index, name, capture_tile))
        .collect();
    for (stream, issues, fatal) in read {
        scan.streams.push(stream);
        scan.issues.extend(issues);
        scan.fatal = scan.fatal.take().or(fatal);
    }
    scan
}

fn read_stream(
    archive: &Archive,
    index: &p4kx_extraction::PathIndex<'_>,
    name: &str,
    capture_tile: bool,
) -> (StreamScan, Vec<IssueKind>, Option<AssemblyError>) {
    let mut stream = StreamScan {
        name: name.to_owned(),
        objects: None,
        entities: None,
        origin: None,
    };
    let Some(entry) = is_plain(name).then(|| index.find(name)).flatten() else {
        let file = name.to_owned();
        return (stream, vec![IssueKind::FileMissing { file }], None);
    };
    match read_file(archive, entry) {
        Err(FileFailure::Fatal(error)) => {
            (stream, Vec::new(), Some(AssemblyError::Provider(error)))
        }
        Err(failure) => {
            let issue = IssueKind::FileUnreadable {
                file: entry.path().to_owned(),
                reason: failure.reason(),
            };
            (stream, vec![issue], None)
        }
        Ok(bytes) => {
            let mut issues = Vec::new();
            let unreadable = |reason: String| IssueKind::FileUnreadable {
                file: entry.path().to_owned(),
                reason,
            };
            match IncludedObjects::read(&bytes) {
                Ok(objects) => stream.objects = objects,
                Err(error) => issues.push(unreadable(error.to_string())),
            }
            match SocEntities::read(&bytes) {
                Ok(entities) => {
                    if capture_tile && entities.is_some() {
                        stream.origin = archive.entry_origin(entry).map(Arc::new);
                    }
                    stream.entities = entities;
                }
                Err(SocEntitiesError::Chunks(_)) if !issues.is_empty() => {}
                Err(error) => issues.push(unreadable(describe(&error))),
            }
            (stream, issues, None)
        }
    }
}

fn describe(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut cause = std::error::Error::source(error);
    while let Some(next) = cause {
        text.push_str(": ");
        text.push_str(&next.to_string());
        cause = next.source();
    }
    text
}

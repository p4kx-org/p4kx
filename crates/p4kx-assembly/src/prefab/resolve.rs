// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use p4kx_model::{PrefabRef, Transform};

use super::resolution_source::OwnedPrefabLocation;
use super::set::Lookup;
use super::{
    MAX_PREFAB_DEPTH, MAX_PREFAB_ISSUES, MAX_PREFAB_OBJECTS, MAX_PREFAB_VISITS, PrefabError,
    PrefabFile, PrefabInstanceSource, PrefabIssue, PrefabIssueKind, PrefabIssueSource,
    PrefabLinkSource, PrefabResolutionSource, PrefabSelectionSource, PrefabSet, ResolvedObject,
    ResolvedPrefab,
};

impl PrefabSet<'_> {
    /// Resolves the prefab `prefab` of the library `library`: its objects with their transforms
    /// to the prefab's space, every nested prefab expanded in place of the object that instances
    /// it with the object's transform composed with those of its objects.
    ///
    /// A prefab that contains itself, that is nested deeper than [`MAX_PREFAB_DEPTH`] or that no
    /// library holds is not expanded: its object stays in the list and an issue says why.
    ///
    /// # Errors
    ///
    /// Returns [`PrefabError`] when the library or the prefab does not exist, the library cannot
    /// be read or the Archive needs a key.
    pub fn resolve(&mut self, library: &str, prefab: &str) -> Result<ResolvedPrefab, PrefabError> {
        let file = self.library(library)?;
        let Some(index) = file.find(prefab) else {
            return Err(PrefabError::NoSuchPrefab {
                library: library.to_owned(),
                name: prefab.to_owned(),
            });
        };
        let capture = self.captures_objects().then(|| Capture::new(&file, index));
        let mut walk = Walk {
            set: self,
            capture,
            objects: Vec::new(),
            issues: Vec::new(),
            visits: 0,
            prefabs: 0,
            stack: Vec::new(),
            reported: HashSet::new(),
            mismatched: HashSet::new(),
            stopped: false,
        };
        let root: Arc<str> = Arc::from("");
        let visit = Visit {
            file: &file,
            index,
            container: &root,
            route: None,
        };
        walk.expand(&visit, Transform::IDENTITY)?;
        let name = file
            .library()
            .prefabs
            .get(index)
            .map_or_else(|| prefab.to_owned(), |found| found.name.clone());
        Ok(ResolvedPrefab {
            file: walk.set.shown(file.slot()),
            library: file.library().name.clone(),
            name,
            objects: walk.objects,
            prefabs: walk.prefabs,
            issues: walk.issues,
            source: walk.capture.map(|capture| capture.source),
        })
    }
}

type ObjectKey = (usize, usize, usize);

struct Capture {
    source: PrefabResolutionSource,
    selections: HashMap<ObjectKey, Arc<PrefabSelectionSource>>,
}

impl Capture {
    fn new(file: &Arc<PrefabFile>, prefab_index: usize) -> Self {
        Self {
            source: PrefabResolutionSource {
                root: OwnedPrefabLocation {
                    file: Arc::clone(file),
                    prefab_index,
                },
                entered: Vec::new(),
                issues: Vec::new(),
            },
            selections: HashMap::new(),
        }
    }

    fn record_entered(&mut self, route: &PrefabInstanceSource) {
        let selection = &route.selection;
        let key = (
            selection.owner.file.slot(),
            selection.owner.prefab_index,
            selection.object_index,
        );
        if let Entry::Vacant(entry) = self.selections.entry(key) {
            entry.insert(Arc::clone(selection));
            self.source.entered.push(Arc::clone(selection));
        }
    }
}

struct Visit<'a> {
    file: &'a Arc<PrefabFile>,
    index: usize,
    container: &'a Arc<str>,
    route: Option<&'a Arc<PrefabInstanceSource>>,
}

impl Visit<'_> {
    fn owner(&self) -> OwnedPrefabLocation {
        OwnedPrefabLocation {
            file: Arc::clone(self.file),
            prefab_index: self.index,
        }
    }

    fn key(&self, object: usize) -> ObjectKey {
        (self.file.slot(), self.index, object)
    }
}

#[derive(Default)]
struct NestedDecision {
    target: Option<(Arc<PrefabFile>, usize)>,
    source: Option<Arc<PrefabLinkSource>>,
}

struct Walk<'s, 'a> {
    set: &'s mut PrefabSet<'a>,
    capture: Option<Capture>,
    objects: Vec<ResolvedObject>,
    issues: Vec<PrefabIssue>,
    visits: usize,
    prefabs: usize,
    stack: Vec<(usize, usize)>,
    reported: HashSet<(usize, usize)>,
    mismatched: HashSet<(usize, usize, String)>,
    stopped: bool,
}

impl Walk<'_, '_> {
    fn note(
        &mut self,
        visit: &Visit<'_>,
        object_index: Option<usize>,
        name: &str,
        kind: PrefabIssueKind,
        link: Option<&Arc<PrefabLinkSource>>,
    ) {
        if self.issues.len() > MAX_PREFAB_ISSUES {
            return;
        }
        let kind = if self.issues.len() == MAX_PREFAB_ISSUES {
            PrefabIssueKind::TooManyIssues {
                limit: MAX_PREFAB_ISSUES,
            }
        } else {
            kind
        };
        self.issues.push(PrefabIssue {
            container: visit.container.to_string(),
            name: name.to_owned(),
            kind,
        });
        if let Some(capture) = &mut self.capture {
            capture.source.issues.push(PrefabIssueSource {
                owner: visit.owner(),
                object_index,
                route: visit.route.cloned(),
                link: link.cloned(),
            });
        }
    }

    fn report_prefab(&mut self, visit: &Visit<'_>) {
        let Some(prefab) = visit.file.library().prefabs.get(visit.index) else {
            return;
        };
        if !self.reported.insert((visit.file.slot(), visit.index)) {
            return;
        }
        if prefab.invalid > 0 {
            let kind = PrefabIssueKind::ObjectsLeftOut {
                prefab: prefab.name.clone(),
                count: prefab.invalid,
            };
            self.note(visit, None, "", kind, None);
        }
        let attached = prefab
            .objects
            .iter()
            .filter(|object| object.attached)
            .count();
        if attached > 0 {
            let kind = PrefabIssueKind::Attached {
                prefab: prefab.name.clone(),
                count: attached,
            };
            self.note(visit, None, "", kind, None);
        }
    }

    fn expand(&mut self, visit: &Visit<'_>, world: Transform) -> Result<(), PrefabError> {
        let Some(prefab) = visit.file.library().prefabs.get(visit.index) else {
            return Ok(());
        };
        self.prefabs += 1;
        if let (Some(capture), Some(route)) = (&mut self.capture, visit.route) {
            capture.record_entered(route);
        }
        self.report_prefab(visit);
        self.stack.push((visit.file.slot(), visit.index));
        for (at, object) in prefab.objects.iter().enumerate() {
            if self.stopped {
                break;
            }
            self.visits += 1;
            if self.visits > MAX_PREFAB_VISITS {
                self.stopped = true;
                let kind = PrefabIssueKind::TooManyVisits {
                    limit: MAX_PREFAB_VISITS,
                };
                self.note(visit, None, "", kind, None);
                break;
            }
            let placed = world.then(&object.transform);
            let nested = match &object.nested {
                Some(reference) => self.nested(visit, at, &object.name, reference)?,
                None => NestedDecision::default(),
            };
            match nested.target {
                Some((inner, target)) => {
                    let path: Arc<str> = if visit.container.is_empty() {
                        Arc::from(object.name.as_str())
                    } else {
                        Arc::from(format!("{}/{}", visit.container, object.name))
                    };
                    let route = nested.source.map(|source| self.instance(visit, at, source));
                    let child = Visit {
                        file: &inner,
                        index: target,
                        container: &path,
                        route: route.as_ref(),
                    };
                    self.expand(&child, placed)?;
                }
                None => self.push(visit, at, placed, nested.source),
            }
        }
        self.stack.pop();
        Ok(())
    }

    fn instance(
        &self,
        visit: &Visit<'_>,
        object_index: usize,
        lookup: Arc<PrefabLinkSource>,
    ) -> Arc<PrefabInstanceSource> {
        let selection = self
            .capture
            .as_ref()
            .and_then(|capture| capture.selections.get(&visit.key(object_index)))
            .cloned()
            .unwrap_or_else(|| {
                Arc::new(PrefabSelectionSource {
                    owner: visit.owner(),
                    object_index,
                    lookup,
                })
            });
        Arc::new(PrefabInstanceSource {
            selection,
            parent: visit.route.cloned(),
        })
    }

    fn push(
        &mut self,
        visit: &Visit<'_>,
        object: usize,
        world: Transform,
        unexpanded_source: Option<Arc<PrefabLinkSource>>,
    ) {
        if self.objects.len() >= MAX_PREFAB_OBJECTS {
            if !self.stopped {
                self.stopped = true;
                let kind = PrefabIssueKind::TooManyObjects {
                    limit: MAX_PREFAB_OBJECTS,
                };
                self.note(visit, None, "", kind, None);
            }
            return;
        }
        self.objects.push(ResolvedObject {
            file: Arc::clone(visit.file),
            prefab: visit.index,
            object,
            world,
            container: Arc::clone(visit.container),
            depth: self.stack.len().saturating_sub(1),
            route: visit.route.cloned(),
            unexpanded_source,
        });
    }

    fn nested(
        &mut self,
        visit: &Visit<'_>,
        object: usize,
        name: &str,
        reference: &PrefabRef,
    ) -> Result<NestedDecision, PrefabError> {
        let lookup = self.set.find_nested(reference)?;
        let source = lookup.source;
        let (file, target) = match lookup.outcome {
            Lookup::Found(file, target) => (file, target),
            Lookup::Missing(unreadable) => {
                if let Some((library, reason)) = unreadable {
                    let kind = PrefabIssueKind::LibraryUnreadable { library, reason };
                    self.note(visit, Some(object), name, kind, source.as_ref());
                }
                let kind = PrefabIssueKind::Missing {
                    prefab: reference.name.clone(),
                };
                self.note(visit, Some(object), name, kind, source.as_ref());
                return Ok(NestedDecision {
                    target: None,
                    source,
                });
            }
        };
        if self.stack.contains(&(file.slot(), target)) {
            let kind = PrefabIssueKind::Cycle {
                prefab: reference.name.clone(),
            };
            self.note(visit, Some(object), name, kind, source.as_ref());
            return Ok(NestedDecision {
                target: None,
                source,
            });
        }
        if self.stack.len() >= MAX_PREFAB_DEPTH {
            let kind = PrefabIssueKind::TooDeep {
                limit: MAX_PREFAB_DEPTH,
            };
            self.note(visit, Some(object), name, kind, source.as_ref());
            return Ok(NestedDecision {
                target: None,
                source,
            });
        }
        if let Some(found) = file.library().prefabs.get(target)
            && !reference.guid.is_empty()
            && !found.id.is_empty()
            && !found.id.eq_ignore_ascii_case(&reference.guid)
            && self
                .mismatched
                .insert((file.slot(), target, reference.guid.to_ascii_lowercase()))
        {
            let kind = PrefabIssueKind::GuidMismatch {
                prefab: reference.name.clone(),
                named: reference.guid.clone(),
                found: found.id.clone(),
            };
            self.note(visit, Some(object), name, kind, source.as_ref());
        }
        Ok(NestedDecision {
            target: Some((file, target)),
            source,
        })
    }
}

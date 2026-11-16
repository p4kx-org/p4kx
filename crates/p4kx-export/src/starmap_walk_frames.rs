// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice::Iter;

use p4kx_extraction::CancelToken;

use super::{System, WalkPath};
use crate::CatalogError;
use crate::starmap_containers::instance::ContainerFile;
use crate::starmap_parse::{Body, Child, Level, Pose};
use crate::starmap_report::clip;

enum Frame<'a> {
    Enter {
        entry: usize,
        path: WalkPath,
    },
    Children(ChildCursor<'a>),
    Finish {
        entry: usize,
        label: &'a str,
        before: [usize; 3],
    },
}

struct ChildCursor<'a> {
    remaining: Iter<'a, Child>,
    capped: bool,
    truncated: bool,
    parent: WalkPath,
    label: &'a str,
    file: &'a ContainerFile,
}

impl<'a> ChildCursor<'a> {
    fn new(level: &'a Level, parent: WalkPath, label: &'a str, file: &'a ContainerFile) -> Self {
        Self {
            remaining: level.children.iter(),
            capped: level.capped,
            truncated: level.truncated,
            parent,
            label,
            file,
        }
    }
}

impl<'a> System<'a> {
    /// Composes each authored route while detecting cycles on the active physical ancestry.
    pub(super) fn walk(&mut self, root: usize) -> Result<(), CatalogError> {
        let mut stack = vec![Frame::Enter {
            entry: root,
            path: WalkPath::ROOT,
        }];
        while let Some(frame) = stack.pop() {
            if self.cancel.is_some_and(CancelToken::is_cancelled) {
                break;
            }
            match frame {
                Frame::Enter { entry, path } => self.enter(entry, &path, &mut stack)?,
                Frame::Children(cursor) => self.next_child(cursor, &mut stack)?,
                Frame::Finish {
                    entry,
                    label,
                    before,
                } => {
                    self.open.remove(&entry);
                    self.report_placements(before, label);
                }
            }
        }
        Ok(())
    }

    fn enter(
        &mut self,
        entry: usize,
        path: &WalkPath,
        stack: &mut Vec<Frame<'a>>,
    ) -> Result<(), CatalogError> {
        let entries = self.index.entries;
        let label = entries.get(entry).map_or("", p4kx_extraction::Entry::path);
        if self.open.contains(&entry) {
            self.skip_route(&format!("{label}: names a container that is open above it"));
            return Ok(());
        }
        let bodies = self.bodies;
        match bodies.get(&entry) {
            None => {
                self.read.unreadable += 1;
                self.issue(&format!("{label}: not read"));
            }
            Some(Body::Unreadable(reason)) => {
                self.read.unreadable += 1;
                self.issue(&format!("{label}: {reason}"));
            }
            Some(Body::Unparsed) => {
                self.opened(entry, path.under, path.external);
                self.read.unreadable += 1;
                self.issue(&format!("{label}: the XML cannot be parsed"));
            }
            Some(Body::Parsed {
                root,
                file,
                fallback,
                ..
            }) => {
                let before = self.seen;
                self.opened(entry, path.under, path.external);
                self.record_instance(path, file)?;
                if let Some(taken) = fallback {
                    self.issue(&format!(
                        "{label}: no XML named like the container, read {} instead",
                        clip(taken)
                    ));
                }
                self.open.insert(entry);
                stack.push(Frame::Finish {
                    entry,
                    label,
                    before,
                });
                stack.push(Frame::Children(ChildCursor::new(
                    root,
                    path.within_container(),
                    label,
                    file,
                )));
            }
        }
        Ok(())
    }

    fn next_child(
        &mut self,
        mut cursor: ChildCursor<'a>,
        stack: &mut Vec<Frame<'a>>,
    ) -> Result<(), CatalogError> {
        let label = cursor.label;
        if cursor.capped {
            let limit = self.limits.depth;
            self.skip_route(&format!("{label}: Children nested deeper than {limit}"));
            return Ok(());
        }
        let Some(child) = cursor.remaining.next() else {
            if cursor.truncated {
                let limit = self.limits.children;
                self.skip_route(&format!(
                    "{label}: more than {limit} Children in the containers of the Archive"
                ));
            }
            return Ok(());
        };
        if self.budget == 0 {
            let limit = self.limits.children;
            self.skip_route(&format!("{label}: more than {limit} Children in a system"));
            return Ok(());
        }
        self.budget -= 1;
        self.seen[0] += 1;
        let file = cursor.file;
        let placed = self.place_child(child, &cursor)?;
        stack.push(Frame::Children(cursor));
        if let Some(path) = placed {
            let external = self.external(child, &path, label);
            stack.push(Frame::Children(ChildCursor::new(
                &child.below,
                path,
                label,
                file,
            )));
            stack.extend(external);
        }
        Ok(())
    }

    fn place_child(
        &mut self,
        child: &Child,
        cursor: &ChildCursor<'a>,
    ) -> Result<Option<WalkPath>, CatalogError> {
        let parent = &cursor.parent;
        let file = cursor.file;
        let label = cursor.label;
        let pose = match &child.pose {
            Pose::Valid(pose) => *pose,
            Pose::Malformed { pos, rot, entity } => {
                let occurrence = parent.child(child, None, parent.transform);
                self.record_jump(child, &occurrence, file, None)?;
                self.note_blocked_descendants(child);
                self.skipped(&format!(
                    "{label}: malformed pos {pos} rot {rot} of {entity}"
                ));
                return Ok(None);
            }
        };
        let local = pose.transform();
        let world = parent.transform.place(local.position, local.rotation);
        let mut path = parent.child(child, Some(pose), world);
        let sane = world.is_sane();
        self.record_jump(child, &path, file, sane.then_some(world))?;
        if !sane {
            self.note_blocked_descendants(child);
            self.skipped(&format!(
                "{label}: out of range: pos {:?} places {:?} of {}",
                local.position,
                world.position,
                clip(child.entity_name())
            ));
            return Ok(None);
        }
        self.note(child, &path, parent.under)?;
        self.record_location(child, &mut path, pose, file)?;
        Ok(Some(path))
    }

    fn external(&mut self, child: &Child, parent: &WalkPath, label: &str) -> Option<Frame<'a>> {
        let path = child.external.as_ref()?;
        self.seen[2] += 1;
        let declared = path.as_str();
        if let Some(entry) = self.index.find(declared) {
            return Some(Frame::Enter {
                entry,
                path: parent.external(path),
            });
        }
        self.read.missing += 1;
        self.issue(&format!(
            "{label}: the Archive holds no container {}",
            clip(declared)
        ));
        None
    }
}

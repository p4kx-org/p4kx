// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, ArchiveError, EntryOrigin, ObservedEntryBinding, PathIndex, PathKeys,
};
use p4kx_model::SocContainerDescriptor;

use super::soc_archive_member::{origin, select};
use super::soc_geometry_capture;
use super::{CaptureCheck, SocCaptureError, SocContainerOccurrence, SocMemberSelector};
use crate::ChildContainerPath;

/// An original nested placement and the result of resolving its selector.
#[derive(Debug)]
pub struct SocChildInput {
    /// Physical selector in the parent's descriptor.
    pub selector: SocMemberSelector,
    /// Resolution relative to the authored container occurrence.
    pub state: SocChildState,
}

/// The source result of one original nested placement.
#[derive(Debug)]
#[non_exhaustive]
pub enum SocChildState {
    /// The original selector is omitted, invalid or unusable.
    InvalidSelector,
    /// No Entry in the allowed physical owners matches the original path.
    Missing,
    /// More than one Entry matches the selected path in its owning Archive.
    Ambiguous(Vec<EntryOrigin>),
    /// The physical target is already an active ancestor of this occurrence.
    Cycle(EntryOrigin),
    /// The selected physical target could not be opened.
    Unreadable {
        /// Original target Entry.
        target: EntryOrigin,
        /// The original Archive failure.
        error: ArchiveError,
    },
    /// The target was opened as a distinct logical occurrence.
    Opened {
        /// Index into the capture's occurrence list.
        occurrence: usize,
    },
}

pub(super) fn declarations(descriptor: &SocContainerDescriptor<'_>) -> Vec<SocChildInput> {
    descriptor
        .children()
        .map(|child| SocChildInput {
            selector: SocMemberSelector::from_source(child),
            state: SocChildState::InvalidSelector,
        })
        .collect()
}

pub(super) struct Walk<'a, 'check> {
    given: Vec<(&'a Archive, PathKeys)>,
    active: Vec<Frame>,
    check: CaptureCheck<'check>,
    pub(super) occurrences: Vec<SocContainerOccurrence>,
}

struct Frame {
    owned: Option<(Box<Archive>, PathKeys)>,
    binding: ObservedEntryBinding,
    occurrence: usize,
    next_child: usize,
}

impl<'a, 'check> Walk<'a, 'check> {
    pub(super) fn new(
        layers: &[&'a Archive],
        check: CaptureCheck<'check>,
    ) -> Result<Self, SocCaptureError> {
        let given = layers
            .iter()
            .map(|archive| Ok((*archive, check.keys(archive)?)))
            .collect::<Result<_, SocCaptureError>>()?;
        Ok(Self {
            given,
            active: Vec::new(),
            check,
            occurrences: Vec::new(),
        })
    }

    pub(super) fn visit(&mut self, root: EntryOrigin) -> Result<(), SocCaptureError> {
        let (archive, keys) = self.given.last().ok_or(SocCaptureError::Root)?;
        let binding = root.binding();
        let mut capture = super::scan(&keys.index(archive), root, None, self.check)?;
        soc_geometry_capture::capture(&mut capture, self.geometry_layers(), self.check)?;
        self.occurrences.push(capture);
        self.active.push(Frame {
            owned: None,
            binding,
            occurrence: 0,
            next_child: 0,
        });
        while let Some(frame) = self.active.last_mut() {
            self.check.poll()?;
            let at = (frame.occurrence, frame.next_child);
            frame.next_child += 1;
            let Some(child) = self
                .occurrences
                .get(at.0)
                .and_then(|parent| parent.children.get(at.1))
            else {
                self.active.pop();
                continue;
            };
            let target = match child.selector.path().and_then(ChildContainerPath::new) {
                Some(path) => self.resolve(&path)?,
                None => ChildTarget::Failed(SocChildState::InvalidSelector),
            };
            let (state, frame) = self.enter(at, target)?;
            let child = self
                .occurrences
                .get_mut(at.0)
                .and_then(|parent| parent.children.get_mut(at.1))
                .ok_or(SocCaptureError::Origin)?;
            child.state = state;
            if let Some(frame) = frame {
                self.active.push(frame);
            }
        }
        Ok(())
    }

    fn enter(
        &mut self,
        at: (usize, usize),
        target: ChildTarget,
    ) -> Result<(SocChildState, Option<Frame>), SocCaptureError> {
        let (archive, origin) = match target {
            ChildTarget::Failed(state) => return Ok((state, None)),
            ChildTarget::Opened(archive, origin) => (archive, origin),
        };
        let check = self.check;
        let binding = origin.binding();
        let keys = check.keys(&archive)?;
        let occurrence = self.occurrences.len();
        let index = keys.index(&archive);
        let mut capture = super::scan(&index, origin, Some(at), check)?;
        soc_geometry_capture::capture(
            &mut capture,
            std::iter::once(index).chain(self.geometry_layers()),
            check,
        )?;
        self.occurrences.push(capture);
        Ok((
            SocChildState::Opened { occurrence },
            Some(Frame {
                owned: Some((archive, keys)),
                binding,
                occurrence,
                next_child: 0,
            }),
        ))
    }

    fn geometry_layers(&self) -> impl Iterator<Item = PathIndex<'_>> {
        self.active
            .iter()
            .rev()
            .filter_map(|frame| frame.owned.as_ref())
            .map(|(archive, keys)| keys.index(archive))
            .chain(
                self.given
                    .iter()
                    .rev()
                    .map(|(archive, keys)| keys.index(archive)),
            )
    }

    fn resolve(&self, path: &ChildContainerPath<'_>) -> Result<ChildTarget, SocCaptureError> {
        for (archive, keys) in self
            .active
            .iter()
            .rev()
            .filter_map(|frame| frame.owned.as_ref())
        {
            if let Some(target) = self.select_child(&keys.index(archive), [path.relative()])? {
                return Ok(target);
            }
        }
        for (layer, (archive, keys)) in self.given.iter().enumerate().rev() {
            let index = keys.index(archive);
            let found = if layer == 0 && self.given.len() > 1 {
                self.select_child(&index, path.root_candidates())?
            } else {
                self.select_child(&index, [path.relative()])?
            };
            if let Some(target) = found {
                return Ok(target);
            }
        }
        Ok(ChildTarget::Failed(SocChildState::Missing))
    }

    fn select_child(
        &self,
        index: &PathIndex<'_>,
        paths: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Option<ChildTarget>, SocCaptureError> {
        let archive = index.archive();
        let matches = select(index, paths, || self.check.poll())?;
        let target = match matches.as_slice() {
            [] => return Ok(None),
            [entry] => {
                let target = origin(archive, entry)?;
                if self.is_active(&target.binding()) {
                    ChildTarget::Failed(SocChildState::Cycle(target))
                } else {
                    match archive.open_container(entry) {
                        Ok(archive) => ChildTarget::Opened(Box::new(archive), target),
                        Err(error) => {
                            ChildTarget::Failed(SocChildState::Unreadable { target, error })
                        }
                    }
                }
            }
            entries => {
                let targets = entries
                    .iter()
                    .map(|entry| origin(archive, entry))
                    .collect::<Result<_, _>>()?;
                ChildTarget::Failed(SocChildState::Ambiguous(targets))
            }
        };
        Ok(Some(target))
    }

    fn is_active(&self, binding: &ObservedEntryBinding) -> bool {
        self.active
            .iter()
            .any(|frame| frame.binding.is_same_entry(binding))
            || self
                .given
                .iter()
                .filter_map(|(archive, _)| archive.container_input_origin())
                .any(|origin| origin.binding().is_same_entry(binding))
    }
}

enum ChildTarget {
    Failed(SocChildState),
    Opened(Box<Archive>, EntryOrigin),
}

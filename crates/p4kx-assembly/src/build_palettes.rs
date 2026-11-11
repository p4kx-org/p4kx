// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use p4kx_dataforge::Guid;
use p4kx_model::{MaterialStatus, TintPalette};

use crate::build_models::Loaded;
use crate::flatten::Draft;
use crate::{Geometry, IssueKind, PaletteFailure, PaletteProvider, PaletteRecords};

type Cached = Result<ChosenPalette, PaletteFailure>;

#[derive(Clone)]
pub(crate) struct ChosenPalette {
    pub(crate) palette: Arc<TintPalette>,
    pub(crate) records: PaletteRecords,
}

/// The palette of a part and the one it hands down to its attachments.
#[derive(Default)]
pub(crate) struct Resolved {
    pub(crate) palette: Option<Arc<TintPalette>>,
    pub(crate) handed_down: Option<Arc<TintPalette>>,
}

/// Resolves the tint palette of each part, reading every distinct palette Record once.
///
/// A part takes the palette its Entity's geometry names; when that cannot be resolved, or the
/// Entity names none, the palette of its nearest ancestor, else the default palette of its
/// material library. Palettes are cached by the GUID of the Record, however it was named, so one
/// Record is one palette. Each failure message is reported for the first part that meets it.
pub(crate) struct Palettes<'p> {
    provider: Option<&'p dyn PaletteProvider>,
    cache: HashMap<Guid, Cached>,
    reported: HashSet<String>,
    handed_down: Vec<Option<Arc<TintPalette>>>,
}

impl<'p> Palettes<'p> {
    pub(crate) fn new(provider: Option<&'p dyn PaletteProvider>) -> Self {
        Self {
            provider,
            cache: HashMap::new(),
            reported: HashSet::new(),
            handed_down: Vec::new(),
        }
    }

    /// Returns the palette of the next part `draft`, in the order of the drafts, and what could not
    /// be resolved on the way. A part without a model has no palette and hands its parent's down.
    pub(crate) fn of_part(
        &mut self,
        draft: &Draft<'_>,
        model: Option<&Loaded>,
        library: &MaterialStatus,
    ) -> (Option<Arc<TintPalette>>, Vec<IssueKind>) {
        let inherited = draft
            .parent
            .and_then(|parent| self.handed_down.get(parent))
            .and_then(Option::as_ref)
            .cloned();
        if model.is_none() {
            self.handed_down.push(inherited);
            return (None, Vec::new());
        }
        let (resolved, issues) = self.of(draft.geometry, inherited.as_ref(), library);
        self.handed_down.push(resolved.handed_down);
        (resolved.palette, issues)
    }

    /// Returns the palette of a part drawn with `library` and the one its attachments inherit,
    /// and what could not be resolved on the way.
    ///
    /// The part takes the palette its Entity's geometry names; else `inherited`, the one of its
    /// nearest ancestor; else the default palette of `library`. Only the first two are handed
    /// down: a library default is no palette of the part's own.
    pub(crate) fn of(
        &mut self,
        geometry: Option<&Geometry>,
        inherited: Option<&Arc<TintPalette>>,
        library: &MaterialStatus,
    ) -> (Resolved, Vec<IssueKind>) {
        let mut issues = Vec::new();
        let Some(provider) = self.provider else {
            return (Resolved::default(), issues);
        };
        let own = geometry
            .and_then(Geometry::palette)
            .and_then(|named| self.named(provider, named, &mut issues))
            .map(|chosen| chosen.palette);
        let handed_down = own.or_else(|| inherited.cloned());
        let palette = handed_down.clone().or_else(|| {
            self.library_default(provider, library, &mut issues)
                .map(|chosen| chosen.palette)
        });
        (
            Resolved {
                palette,
                handed_down,
            },
            issues,
        )
    }

    /// Returns the palette of a part drawn with `library` that names the palette `named`, and what
    /// could not be resolved on the way.
    pub(crate) fn of_named(
        &mut self,
        named: Option<&crate::PaletteRef>,
        library: &MaterialStatus,
    ) -> (Option<Arc<TintPalette>>, Vec<IssueKind>) {
        let (chosen, issues) = self.of_named_observed(named, library);
        (chosen.map(|chosen| chosen.palette), issues)
    }

    pub(crate) fn of_named_observed(
        &mut self,
        named: Option<&crate::PaletteRef>,
        library: &MaterialStatus,
    ) -> (Option<ChosenPalette>, Vec<IssueKind>) {
        let mut issues = Vec::new();
        let Some(provider) = self.provider else {
            return (None, issues);
        };
        let palette = named
            .and_then(|named| self.named(provider, named, &mut issues))
            .or_else(|| self.library_default(provider, library, &mut issues));
        (palette, issues)
    }

    fn named(
        &mut self,
        provider: &dyn PaletteProvider,
        named: &crate::PaletteRef,
        issues: &mut Vec<IssueKind>,
    ) -> Option<ChosenPalette> {
        let guid = named.guid();
        let what = format!("the tint palette {guid}");
        let found = if named.child_path().is_empty() {
            self.read(provider, guid)
        } else {
            Err(PaletteFailure::Unreadable {
                reason: format!("the child {} is not supported", named.child_path()),
            })
        };
        match found {
            Ok(palette) => Some(palette),
            Err(failure) => {
                self.note(issues, &what, &failure);
                None
            }
        }
    }

    fn library_default(
        &mut self,
        provider: &dyn PaletteProvider,
        library: &MaterialStatus,
        issues: &mut Vec<IssueKind>,
    ) -> Option<ChosenPalette> {
        let path = library
            .library()
            .and_then(|library| library.default_palette.as_deref())?;
        let what = format!("the default tint palette {path} of the material library");
        match provider.guid_at_observed(path).and_then(|selector| {
            let mut chosen = self.read(provider, selector.value)?;
            chosen.records.selector = selector.record;
            Ok(chosen)
        }) {
            Ok(palette) => Some(palette),
            Err(failure) => {
                self.note(issues, &what, &failure);
                None
            }
        }
    }

    fn read(&mut self, provider: &dyn PaletteProvider, guid: Guid) -> Cached {
        self.cache
            .entry(guid)
            .or_insert_with(|| {
                provider.by_guid_observed(guid).map(|read| ChosenPalette {
                    palette: Arc::new(read.value),
                    records: PaletteRecords {
                        selector: None,
                        palette: read.record,
                    },
                })
            })
            .clone()
    }

    fn note(&mut self, issues: &mut Vec<IssueKind>, what: &str, failure: &PaletteFailure) {
        let reason = match failure {
            PaletteFailure::Missing => format!("{what} does not exist"),
            PaletteFailure::Unreadable { reason } => format!("{what} cannot be used: {reason}"),
        };
        if self.reported.insert(reason.clone()) {
            issues.push(IssueKind::Palette { reason });
        }
    }
}

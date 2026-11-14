// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::sync::Mutex;

use p4kx_extraction::{
    CancelToken, SourceUse, SourceUseRef, SourceUses, SourceValue, SourcedValue,
};
use serde::{Serialize, Serializer};

use super::OriginalDatum;
use crate::health_metadata::{HealthMetadata, HealthTextRef};
use crate::{CatalogError, ExportAccountingError, HealthText, HealthTextView};

/// A temporary storage position and its exact shared value, without source authority.
#[derive(Debug, Clone)]
pub(super) struct CapturedText {
    ordinal: u64,
    text: HealthText,
}

impl CapturedText {
    pub(super) const fn text(&self) -> &HealthText {
        &self.text
    }
}

impl PartialEq for CapturedText {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl Eq for CapturedText {}

struct Atom {
    inputs: BTreeMap<SourceUseRef, Vec<SourceUse>>,
    complete: bool,
}

enum State {
    Open {
        next: u64,
        atoms: BTreeMap<HealthText, u64>,
    },
    Sealed,
}

/// Stores each actual text value once while retaining separate occurrence contributions.
pub(crate) struct HealthTextCapture {
    state: Mutex<State>,
}

/// Canonical storage and temporary-to-public positions for retained occurrences only.
pub(super) struct SealedTexts {
    metadata: HealthMetadata,
    positions: Vec<Option<TextHandle>>,
}

#[derive(Clone)]
struct TextHandle {
    reference: HealthTextRef,
    text: HealthText,
}

impl HealthTextCapture {
    pub(crate) const fn new() -> Self {
        Self {
            state: Mutex::new(State::Open {
                next: 0,
                atoms: BTreeMap::new(),
            }),
        }
    }

    pub(super) fn capture(
        &self,
        fact: OriginalDatum<&str>,
    ) -> Result<OriginalDatum<CapturedText>, CatalogError> {
        fact.try_map(|text| self.intern(text))
    }

    pub(super) fn capture_shared(
        &self,
        fact: OriginalDatum<HealthText>,
    ) -> Result<OriginalDatum<CapturedText>, CatalogError> {
        fact.try_map(|text| self.intern_with(text.as_str(), || text.clone()))
    }

    pub(super) fn retain_text(&self, text: &str) -> Result<CapturedText, CatalogError> {
        self.intern(text)
    }
    fn intern(&self, text: &str) -> Result<CapturedText, CatalogError> {
        self.intern_with(text, || HealthText::new(text))
    }

    fn intern_with(
        &self,
        text: &str,
        create: impl FnOnce() -> HealthText,
    ) -> Result<CapturedText, CatalogError> {
        let mut state = crate::export_evidence_lock::lock(&self.state)?;
        let State::Open { next, atoms } = &mut *state else {
            return Err(ExportAccountingError::HealthCaptureClosed.into());
        };
        let captured = if let Some((text, ordinal)) = atoms.get_key_value(text) {
            CapturedText {
                ordinal: *ordinal,
                text: text.clone(),
            }
        } else {
            let ordinal = *next;
            *next = next
                .checked_add(1)
                .ok_or(ExportAccountingError::CounterOverflow)?;
            let text = create();
            atoms.insert(text.clone(), ordinal);
            CapturedText { ordinal, text }
        };
        drop(state);
        Ok(captured)
    }

    pub(super) fn seal(
        &self,
        sources: &SourceUses,
        cancel: Option<&CancelToken>,
        retained: &[&OriginalDatum<CapturedText>],
    ) -> Result<SealedTexts, CatalogError> {
        checkpoint(cancel)?;
        let mut state = crate::export_evidence_lock::lock(&self.state)?;
        let State::Open { next, atoms } = std::mem::replace(&mut *state, State::Sealed) else {
            return Err(ExportAccountingError::HealthCaptureClosed.into());
        };
        drop(state);
        let retained = retained_atoms(retained, &atoms)?;
        normalize(next, atoms, retained, sources, cancel)
    }
}

fn retained_atoms(
    facts: &[&OriginalDatum<CapturedText>],
    atoms: &BTreeMap<HealthText, u64>,
) -> Result<BTreeMap<u64, Atom>, ExportAccountingError> {
    let mut retained = BTreeMap::new();
    for fact in facts {
        let SourceValue::Present(text) = fact.value() else {
            continue;
        };
        if atoms.get(text.text.as_str()) != Some(&text.ordinal) {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        let atom = retained.entry(text.ordinal).or_insert_with(|| Atom {
            inputs: BTreeMap::new(),
            complete: true,
        });
        let (inputs, complete) = fact.contributions();
        retain(&mut atom.inputs, inputs);
        atom.complete &= complete;
    }
    Ok(retained)
}

fn normalize(
    count: u64,
    atoms: BTreeMap<HealthText, u64>,
    mut retained: BTreeMap<u64, Atom>,
    sources: &SourceUses,
    cancel: Option<&CancelToken>,
) -> Result<SealedTexts, CatalogError> {
    let count = usize::try_from(count).map_err(|_| ExportAccountingError::CounterOverflow)?;
    let mut positions = vec![None; count];
    let mut values = Vec::with_capacity(retained.len());
    for (text, ordinal) in atoms {
        checkpoint(cancel)?;
        let Some(atom) = retained.remove(&ordinal) else {
            continue;
        };
        let index =
            u64::try_from(values.len()).map_err(|_| ExportAccountingError::CounterOverflow)?;
        let at = usize::try_from(ordinal).map_err(|_| ExportAccountingError::CounterOverflow)?;
        let slot = positions
            .get_mut(at)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if slot
            .replace(TextHandle {
                reference: HealthTextRef::new(index),
                text: text.clone(),
            })
            .is_some()
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let inputs: Vec<_> = atom.inputs.into_values().flatten().collect();
        values.push(sources.value(SourceValue::Present(text), &inputs, atom.complete));
    }
    if !retained.is_empty() {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    Ok(SealedTexts {
        metadata: HealthMetadata::new(values),
        positions,
    })
}

impl SealedTexts {
    pub(super) fn cell(
        &self,
        fact: OriginalDatum<CapturedText>,
        sources: &SourceUses,
    ) -> Result<HealthTextCell, ExportAccountingError> {
        let resolved = fact
            .try_map(|captured| {
                let index = usize::try_from(captured.ordinal)
                    .map_err(|_| ExportAccountingError::CounterOverflow)?;
                let handle = self
                    .positions
                    .get(index)
                    .and_then(Option::as_ref)
                    .filter(|handle| handle.text == captured.text)
                    .cloned()
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                Ok::<_, ExportAccountingError>(handle)
            })?
            .into_cell(sources);
        let text = resolved.value().map(|handle| handle.text.clone());
        Ok(HealthTextCell {
            cell: resolved.map(|handle| handle.reference),
            text,
        })
    }

    pub(super) fn into_metadata(self) -> HealthMetadata {
        self.metadata
    }
}

fn retain(retained: &mut BTreeMap<SourceUseRef, Vec<SourceUse>>, inputs: &[SourceUse]) {
    for input in inputs {
        let group = retained.entry(input.reference().clone()).or_default();
        if !group
            .iter()
            .any(|original| std::ptr::eq(original.description(), input.description()))
        {
            group.push(input.clone());
        }
    }
}

fn checkpoint(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    Ok(())
}

#[cfg(test)]
#[path = "health_text_capture_tests.rs"]
mod tests;

/// One sealed occurrence and its directly paired shared text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HealthTextCell {
    cell: SourcedValue<HealthTextRef>,
    text: Option<HealthText>,
}

impl HealthTextCell {
    pub(super) fn absent(sources: &SourceUses) -> Self {
        Self {
            cell: sources.value(SourceValue::Absent, &[], true),
            text: None,
        }
    }

    pub(super) const fn cell(&self) -> &SourcedValue<HealthTextRef> {
        &self.cell
    }

    pub(super) const fn value(&self) -> Option<&HealthText> {
        self.text.as_ref()
    }

    pub(super) fn view(&self) -> HealthTextView<'_> {
        HealthTextView::new(
            self.text.as_ref().map(HealthText::as_str),
            self.cell.state(),
            self.cell.origin(),
        )
    }
}

impl Serialize for HealthTextCell {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.cell.serialize(serializer)
    }
}

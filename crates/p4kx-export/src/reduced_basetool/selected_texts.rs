// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{CancelToken, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use crate::catalog_query_session::checkpoint;
use crate::catalog_text_contract::{CanonicalText, GameTextRef, PublicNameRef};

#[derive(Default)]
pub(super) struct TextRefs<'source> {
    names: BTreeSet<&'source str>,
    descriptions: BTreeSet<&'source str>,
}

impl<'source> TextRefs<'source> {
    pub(super) fn name(&mut self, key: &'source SourcedValue<PublicNameRef>) {
        if let Some(key) = key.value() {
            self.names.insert(key.as_str());
        }
    }

    pub(super) fn description(&mut self, key: &'source SourcedValue<GameTextRef>) {
        if let Some(key) = key.value() {
            self.descriptions.insert(key.as_str());
        }
    }
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SelectedTexts {
    #[serde(serialize_with = "text_rows")]
    public_names: BTreeMap<String, TextRow>,
    #[serde(serialize_with = "text_rows")]
    game_texts: BTreeMap<String, TextRow>,
}

impl SelectedTexts {
    pub(super) fn add(
        &mut self,
        selected: TextRefs<'_>,
        names: &[CanonicalText<'_, PublicNameRef>],
        descriptions: &[CanonicalText<'_, GameTextRef>],
        cancel: Option<&CancelToken>,
    ) -> Result<(), ReducedError> {
        add_rows(
            &mut self.public_names,
            selected.names,
            names,
            PublicNameRef::as_str,
            cancel,
        )?;
        add_rows(
            &mut self.game_texts,
            selected.descriptions,
            descriptions,
            GameTextRef::as_str,
            cancel,
        )
    }
}

fn text_rows<S: serde::Serializer>(
    rows: &BTreeMap<String, TextRow>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(rows.values())
}

fn add_rows<R>(
    rows: &mut BTreeMap<String, TextRow>,
    mut selected: BTreeSet<&str>,
    canonical: &[CanonicalText<'_, R>],
    key: impl Fn(&R) -> &str,
    cancel: Option<&CancelToken>,
) -> Result<(), ReducedError> {
    use std::collections::btree_map::Entry;

    for text in canonical {
        checkpoint(cancel)?;
        let id = text
            .id()
            .value()
            .map(&key)
            .ok_or(ReducedError::IncompleteScope("texts"))?;
        if selected.remove(id) {
            let reduced = TextRow::project(text, id);
            match rows.entry(id.to_owned()) {
                Entry::Occupied(mut row) => row.get_mut().join(&reduced)?,
                Entry::Vacant(row) => {
                    row.insert(reduced);
                }
            }
        }
    }
    if !selected.is_empty() {
        return Err(ReducedError::IncompleteScope("texts"));
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TextRow {
    id: Cell<String>,
    key: Cell<String>,
    translations: Cell<BTreeMap<String, Forms>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Forms {
    source_text: Cell<String>,
    display_text: Cell<String>,
}

impl TextRow {
    fn project<R>(text: &CanonicalText<'_, R>, id: &str) -> Self {
        let id = Cell::from(text.id()).map(|_| id.to_owned());
        let key = Cell::from(text.key()).map(|key| (*key).to_owned());
        let translations = Cell::from(text.translations()).map(|rows| {
            rows.iter()
                .map(|(language, forms)| {
                    (
                        language.clone(),
                        Forms {
                            source_text: Cell::from(&forms.source_text)
                                .map(|value| value.to_string()),
                            display_text: Cell::from(&forms.display_text)
                                .map(|value| value.to_string()),
                        },
                    )
                })
                .collect()
        });
        Self {
            id,
            key,
            translations,
        }
    }

    fn join(&mut self, incoming: &Self) -> Result<(), ReducedError> {
        join_cell(&mut self.id, &incoming.id)?;
        join_cell(&mut self.key, &incoming.key)?;
        join_marker(&mut self.translations, &incoming.translations)?;
        match (&mut self.translations.value, &incoming.translations.value) {
            (Some(existing), Some(incoming)) if existing.keys().eq(incoming.keys()) => {
                for (existing, incoming) in existing.values_mut().zip(incoming.values()) {
                    join_cell(&mut existing.source_text, &incoming.source_text)?;
                    join_cell(&mut existing.display_text, &incoming.display_text)?;
                }
            }
            (None, None) => {}
            _ => return Err(ReducedError::IncompleteScope("texts")),
        }
        Ok(())
    }
}

fn join_cell<T: PartialEq>(existing: &mut Cell<T>, incoming: &Cell<T>) -> Result<(), ReducedError> {
    if existing.value != incoming.value {
        return Err(ReducedError::IncompleteScope("texts"));
    }
    join_marker(existing, incoming)
}

fn join_marker<T>(existing: &mut Cell<T>, incoming: &Cell<T>) -> Result<(), ReducedError> {
    if existing.state != incoming.state {
        return Err(ReducedError::IncompleteScope("texts"));
    }
    existing.contains_encrypted_source = match (
        existing.contains_encrypted_source,
        incoming.contains_encrypted_source,
    ) {
        (Some(true), _) | (_, Some(true)) => Some(true),
        (Some(false), Some(false)) => Some(false),
        _ => None,
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::SourceValueState;

    use super::super::cell::Cell;
    use super::{ReducedError, join_cell};

    #[test]
    fn duplicate_text_keeps_the_union_of_selected_markers() -> Result<(), ReducedError> {
        let cell = |value, marker| Cell {
            state: SourceValueState::Present,
            value: Some(value),
            contains_encrypted_source: marker,
        };
        let mut combined = cell("ship", Some(false));
        join_cell(&mut combined, &cell("ship", None))?;
        assert_eq!(combined.contains_encrypted_source, None);
        join_cell(&mut combined, &cell("ship", Some(true)))?;
        assert_eq!(combined.contains_encrypted_source, Some(true));
        assert!(
            join_cell(&mut combined, &cell("vehicle", Some(false))).is_err(),
            "two projections of the same canonical text must agree"
        );
        Ok(())
    }
}

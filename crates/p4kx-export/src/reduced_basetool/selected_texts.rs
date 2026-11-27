// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{CancelToken, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::{self, Cell};
use crate::catalog_queries::CatalogPublicName;
use crate::catalog_query_session::checkpoint;
use crate::catalog_text_contract::{CanonicalText, GameTextRef, PublicNameRef, TextForms};

#[derive(Default)]
pub(super) struct TextRefs<'source> {
    names: BTreeSet<&'source str>,
    descriptions: BTreeSet<&'source str>,
}

impl<'source> TextRefs<'source> {
    pub(super) fn name(&mut self, key: Option<&'source PublicNameRef>) {
        if let Some(key) = key {
            self.names.insert(key.as_str());
        }
    }

    pub(super) fn game_text(&mut self, key: Option<&'source GameTextRef>) {
        if let Some(key) = key {
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
            names.iter().map(TextInput::from),
            PublicNameRef::as_str,
            cancel,
        )?;
        add_rows(
            &mut self.game_texts,
            selected.descriptions,
            descriptions.iter().map(TextInput::from),
            GameTextRef::as_str,
            cancel,
        )
    }

    pub(super) fn add_owned_names(
        &mut self,
        selected: TextRefs<'_>,
        names: &[CatalogPublicName],
        cancel: Option<&CancelToken>,
    ) -> Result<(), ReducedError> {
        if !selected.descriptions.is_empty() {
            return Err(ReducedError::IncompleteScope("texts.gameTexts"));
        }
        add_rows(
            &mut self.public_names,
            selected.names,
            names.iter().map(TextInput::from),
            PublicNameRef::as_str,
            cancel,
        )
    }
}

struct TextInput<'read, 'source, R, K> {
    id: &'read SourcedValue<R>,
    key: &'read SourcedValue<K>,
    translations: &'read SourcedValue<BTreeMap<String, TextForms<'source>>>,
}

impl<'read, 'source, R> From<&'read CanonicalText<'source, R>>
    for TextInput<'read, 'source, R, &'source str>
{
    fn from(text: &'read CanonicalText<'source, R>) -> Self {
        Self {
            id: text.id(),
            key: text.key(),
            translations: text.translations(),
        }
    }
}

impl<'read> From<&'read CatalogPublicName> for TextInput<'read, 'static, PublicNameRef, String> {
    fn from(text: &'read CatalogPublicName) -> Self {
        Self {
            id: &text.id,
            key: &text.key,
            translations: &text.translations,
        }
    }
}

fn text_rows<S: serde::Serializer>(
    rows: &BTreeMap<String, TextRow>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(rows.values())
}

fn add_rows<'read, 'source: 'read, R: 'read, K: AsRef<str> + 'read>(
    rows: &mut BTreeMap<String, TextRow>,
    mut selected: BTreeSet<&str>,
    canonical: impl IntoIterator<Item = TextInput<'read, 'source, R, K>>,
    key: impl Fn(&R) -> &str,
    cancel: Option<&CancelToken>,
) -> Result<(), ReducedError> {
    use std::collections::btree_map::Entry;

    for text in canonical {
        checkpoint(cancel)?;
        let id = text
            .id
            .value()
            .map(&key)
            .ok_or(ReducedError::IncompleteScope("texts"))?;
        if selected.remove(id) {
            let reduced = TextRow::project(&text, id);
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
    fn project<R, K: AsRef<str>>(text: &TextInput<'_, '_, R, K>, id: &str) -> Self {
        let id = Cell::from(text.id).map(|_| id.to_owned());
        let key = Cell::from(text.key).map(|key| key.as_ref().to_owned());
        let translations = Cell::from(text.translations).map(|rows| {
            rows.iter()
                .map(|(language, forms)| {
                    (
                        language.clone(),
                        Forms {
                            source_text: Cell::from(&forms.source_text).map(ToString::to_string),
                            display_text: Cell::from(&forms.display_text).map(ToString::to_string),
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
    existing.contains_encrypted_source = cell::merge_encryption(
        existing.contains_encrypted_source,
        incoming.contains_encrypted_source,
    );
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

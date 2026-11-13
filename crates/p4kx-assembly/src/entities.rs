// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_dataforge::{DataForge, Guid, Record};

use crate::EntityLookupError;

/// The type of every Entity Record.
pub const ENTITY_TYPE: &str = "EntityClassDefinition";

/// The prefix of an Entity's Record name before its class name.
const NAME_PREFIX: &str = "EntityClassDefinition.";

/// Finds the Entities of a DataForge by record path, GUID or class name.
///
/// The class names are indexed once on creation; a class name is the Record name after
/// `EntityClassDefinition.`, matched ignoring ASCII case.
#[derive(Debug)]
pub struct Entities<'f> {
    forge: &'f DataForge<'f>,
    classes: HashMap<Box<str>, Vec<Record<'f>>>,
}

impl<'f> Entities<'f> {
    /// Indexes the Entities of `forge`.
    #[must_use]
    pub fn new(forge: &'f DataForge<'f>) -> Self {
        let mut classes: HashMap<Box<str>, Vec<Record<'f>>> = HashMap::new();
        for record in forge.records() {
            if let Some(class) = record.name().ok().and_then(class_name_of)
                && is_entity(&record)
            {
                classes
                    .entry(class.to_ascii_lowercase().into_boxed_str())
                    .or_default()
                    .push(record);
            }
        }
        Self { forge, classes }
    }

    /// Returns the DataForge the Entities belong to.
    #[must_use]
    pub const fn forge(&self) -> &'f DataForge<'f> {
        self.forge
    }

    /// Finds the Entity `text` names: a GUID `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`, a record
    /// path (holding `/` or `\`, ignoring case and a trailing `.xml`), or a class name such as
    /// `AEGS_Avenger_Titan` or the Record name `EntityClassDefinition.AEGS_Avenger_Titan`,
    /// ignoring ASCII case.
    ///
    /// # Errors
    ///
    /// Returns [`EntityLookupError::NotFound`] when nothing matches,
    /// [`EntityLookupError::Ambiguous`] when several Entities do, and
    /// [`EntityLookupError::NotAnEntity`] when the GUID names a Record of another type.
    pub fn find(&self, text: &str) -> Result<Record<'f>, EntityLookupError> {
        let text = text.trim();
        let matches: Vec<Record<'f>> = if let Ok(guid) = text.parse::<Guid>() {
            let record = self.forge.record_by_guid(guid);
            if let Some(record) = record
                && !is_entity(&record)
            {
                return Err(EntityLookupError::NotAnEntity {
                    text: text.to_owned(),
                    type_name: record.type_name().unwrap_or_default().to_owned(),
                });
            }
            record.into_iter().collect()
        } else if text.contains(['/', '\\']) {
            self.forge.records_at_path(text).filter(is_entity).collect()
        } else {
            self.named_class_candidates(text).to_vec()
        };
        match matches.as_slice() {
            [record] => Ok(*record),
            [] => Err(EntityLookupError::NotFound {
                text: text.to_owned(),
            }),
            several => Err(EntityLookupError::Ambiguous {
                text: text.to_owned(),
                candidates: several.iter().map(describe).collect(),
            }),
        }
    }

    /// Returns the first Entity in definition order with the class name `name`, ignoring ASCII
    /// case, or `None` when there is none.
    #[must_use]
    pub fn by_class_name(&self, name: &str) -> Option<Record<'f>> {
        self.class_candidates(name).first().copied()
    }

    pub(crate) fn named_class_candidates(&self, text: &str) -> &[Record<'f>] {
        self.class_candidates(strip_prefix_ignoring_case(text).unwrap_or(text))
    }

    /// Returns every Entity with the class name `name` in definition order, ignoring ASCII case.
    pub(crate) fn class_candidates(&self, name: &str) -> &[Record<'f>] {
        self.classes
            .get(name.to_ascii_lowercase().as_str())
            .map_or(&[], Vec::as_slice)
    }
}

/// Returns whether the Record is an Entity.
pub(crate) fn is_entity(record: &Record<'_>) -> bool {
    record.type_name().is_ok_and(|name| name == ENTITY_TYPE)
}

/// Returns the class name in an Entity's Record name, or `None` for any other name.
pub(crate) fn class_name_of(name: &str) -> Option<&str> {
    name.strip_prefix(NAME_PREFIX)
}

/// Returns the class name in a Record name typed in any case, or `None` without the prefix.
fn strip_prefix_ignoring_case(text: &str) -> Option<&str> {
    let prefix = text.get(..NAME_PREFIX.len())?;
    prefix
        .eq_ignore_ascii_case(NAME_PREFIX)
        .then(|| text.get(NAME_PREFIX.len()..))
        .flatten()
}

/// Returns the GUID and record path of a Record, to name it among candidates.
pub(crate) fn describe(record: &Record<'_>) -> String {
    format!(
        "{} ({})",
        record.guid(),
        record.record_path().unwrap_or_default()
    )
}

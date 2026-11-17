// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::catalog_queries::{
    CanonicalText, ClueDetails, ContractSnapshot, GameTextRef, Investigation, MissionPropertyText,
    NpcSpawnName, PropertyValue,
};
use p4kx_extraction::{FieldOrigin, SourceLocation, SourcedValue};

use super::fixture;
use crate::catalog_contracts::TestResult;

pub(super) fn verify(snapshot: &ContractSnapshot<'_>, outside: u32, selected: u32) -> TestResult {
    let mut check = TextCheck {
        snapshot,
        outside,
        selected,
        keys: BTreeSet::new(),
    };
    check.property_sets()?;
    check.npc_names()?;
    check.contract_texts()?;
    let expected: BTreeSet<_> = fixture::TEXTS.iter().map(|(key, _)| *key).collect();
    assert_eq!(
        check.keys, expected,
        "all authored protected keys resolve in the selective snapshot"
    );
    let table_keys: BTreeSet<_> = snapshot
        .game_texts
        .iter()
        .map(|text| {
            text.key()
                .value()
                .and_then(|key| key.strip_prefix('@'))
                .ok_or("a complete authored table key")
        })
        .collect::<Result<_, _>>()?;
    assert_eq!(
        table_keys, check.keys,
        "the returned text table belongs to the actual published occurrences"
    );
    Ok(())
}

struct TextCheck<'snapshot, 'source> {
    snapshot: &'snapshot ContractSnapshot<'source>,
    outside: u32,
    selected: u32,
    keys: BTreeSet<&'source str>,
}

impl<'source> TextCheck<'_, 'source> {
    fn property_sets(&mut self) -> TestResult {
        let snapshot = self.snapshot;
        assert!(
            !snapshot.mission_property_sets.is_empty(),
            "selected rows retain canonical property sets"
        );
        for set in &snapshot.mission_property_sets {
            for variable in ["Name", "Clues", "Number", "Crew"] {
                assert!(
                    set.properties
                        .iter()
                        .flatten()
                        .any(|property| property.variable.as_deref() == Some(variable)),
                    "the retained canonical alias keeps each typed property declaration"
                );
            }
            for property in set.properties.iter().flatten() {
                self.property(
                    property
                        .value
                        .as_ref()
                        .ok_or("the original typed property")?,
                )?;
            }
        }
        Ok(())
    }

    fn npc_names(&mut self) -> TestResult {
        let snapshot = self.snapshot;
        let pool = snapshot
            .npc_spawns
            .iter()
            .find(|pool| {
                pool.groups
                    .iter()
                    .any(|group| group.name.as_deref() == Some("Crew"))
            })
            .ok_or("the retained named NPC pool")?;
        for option in pool.groups.iter().flat_map(|group| &group.options) {
            let name = option
                .embedded_name
                .as_ref()
                .ok_or("the retained embedded NPC name")?;
            for cell in [
                &name.given_name,
                &name.given_last_name,
                &name.given_nick_name,
            ] {
                self.cell(cell, true)?;
            }
        }
        Ok(())
    }

    fn contract_texts(&mut self) -> TestResult {
        let snapshot = self.snapshot;
        for row in &snapshot.rows {
            self.cell(&row.base.contractor, false)?;
            if let Some(cell) = &row.base.mission_type {
                self.cell(cell, false)?;
            }
        }
        for title in &snapshot.mission_titles {
            self.cell(&title.title.key, false)?;
            self.cell(&title.description.key, false)?;
        }
        Ok(())
    }

    fn property(&mut self, value: &PropertyValue<'source>) -> TestResult {
        match value {
            PropertyValue::AiName(name) => self.name(name)?,
            PropertyValue::Investigation(investigation) => self.investigation(investigation)?,
            PropertyValue::Integer { options } => {
                for option in options.iter().flatten().flatten() {
                    self.cell(
                        option
                            .metadata
                            .display_name
                            .as_ref()
                            .ok_or("the published option display name")?,
                        true,
                    )?;
                }
            }
            PropertyValue::NpcSpawnDescriptions { .. } => {}
            _ => return Err("an unexpected fixture property variant".into()),
        }
        Ok(())
    }

    fn name(&mut self, name: &NpcSpawnName<MissionPropertyText<'source>>) -> TestResult {
        for text in [
            &name.given_name,
            &name.given_last_name,
            &name.given_nick_name,
        ] {
            self.cell(
                text.published()
                    .ok_or("a retained property text remained pending")?,
                true,
            )?;
        }
        Ok(())
    }

    fn investigation(&mut self, investigation: &Investigation<'source>) -> TestResult {
        for suspect in investigation.suspects.iter().flatten().flatten() {
            self.name(
                &suspect
                    .name
                    .as_ref()
                    .ok_or("the authored suspect name")?
                    .name,
            )?;
        }
        for location in investigation.locations.iter().flatten().flatten() {
            for item in location.clue_items.iter().flatten().flatten() {
                for clue in item.clues.iter().flatten().flatten() {
                    self.published(&clue.objective_long_description)?;
                    self.published(&clue.objective_short_description)?;
                    let ClueDetails::CorpseScanInfo {
                        name,
                        cause_of_death,
                    } = &clue.details
                    else {
                        return Err("the authored scan clue".into());
                    };
                    self.name(&name.as_ref().ok_or("the authored scan name")?.name)?;
                    self.published(cause_of_death)?;
                }
            }
        }
        Ok(())
    }

    fn published(&mut self, text: &MissionPropertyText<'source>) -> TestResult {
        self.cell(
            text.published()
                .ok_or("a retained property text remained pending")?,
            true,
        )
    }

    fn cell(&mut self, cell: &SourcedValue<GameTextRef>, outside_owner: bool) -> TestResult {
        self.origin(cell.origin());
        if outside_owner {
            assert!(
                cell.value().is_some(),
                "every authored fixture text has a published reference"
            );
        }
        let Some(reference) = cell.value() else {
            return Ok(());
        };
        assert_eq!(
            cell.origin().contains_encrypted_source(),
            Some(false),
            "the original text reference comes from the plain DataForge"
        );
        assert!(
            cell.origin().source_set_complete(),
            "the original key occurrence is complete"
        );
        if outside_owner {
            assert!(
                self.cites_owner(cell.origin(), self.outside),
                "the retained canonical payload cites the earlier outside-root owner"
            );
            assert!(
                !self.cites_owner(cell.origin(), self.selected),
                "the later selected alias does not replace the canonical text occurrence"
            );
        }
        let snapshot = self.snapshot;
        let text = snapshot
            .game_texts
            .iter()
            .find(|text| text.id().value() == Some(reference))
            .ok_or("the snapshot's protected text reference")?;
        self.definition(text)
    }

    fn definition(&mut self, text: &CanonicalText<'source, GameTextRef>) -> TestResult {
        for origin in [text.id().origin(), text.key().origin()] {
            self.origin(origin);
            assert_eq!(
                origin.contains_encrypted_source(),
                Some(false),
                "the canonical key identity keeps the original plain DataForge origin"
            );
        }
        let key = *text.key().value().ok_or("the authored canonical key")?;
        let unprefixed = key
            .strip_prefix('@')
            .ok_or("an original localization key")?;
        let expected: &str = fixture::TEXTS
            .iter()
            .find(|(candidate, _)| *candidate == unprefixed)
            .map(|(_, value)| *value)
            .ok_or("an authored fixture key")?;
        self.forms(text, expected)?;
        self.keys.insert(unprefixed);
        Ok(())
    }

    fn forms(&self, text: &CanonicalText<'source, GameTextRef>, expected: &str) -> TestResult {
        let translations = text.translations();
        self.origin(translations.origin());
        assert_eq!(
            translations.origin().contains_encrypted_source(),
            Some(true),
            "translated membership includes the encrypted locale actually consulted"
        );
        let english = translations
            .value()
            .and_then(|languages| languages.get("en"))
            .ok_or("the requested English forms")?;
        for form in [&english.source_text, &english.display_text] {
            assert_eq!(
                form.value().map(AsRef::as_ref),
                Some(expected),
                "the encrypted locale provides the expected complete text"
            );
            assert_eq!(
                form.origin().contains_encrypted_source(),
                Some(true),
                "each translated datum independently carries the encrypted locale marker"
            );
            assert!(
                form.origin().source_set_complete(),
                "the translated datum's inputs are known"
            );
            self.origin(form.origin());
        }
        Ok(())
    }

    fn cites_owner(&self, origin: &FieldOrigin, owner: u32) -> bool {
        origin.source_uses_refs().iter().any(|reference| {
            self.snapshot.source_inventory.iter_uses().any(|used| {
                used.id() == reference
                    && match used.location() {
                        SourceLocation::Subresource {
                            namespace, indices, ..
                        } => {
                            namespace == "dataforgeValue"
                                && indices.first() == Some(&u64::from(owner))
                        }
                        SourceLocation::DataForge { record_index, .. } => *record_index == owner,
                        _ => false,
                    }
            })
        })
    }

    fn origin(&self, origin: &FieldOrigin) {
        for reference in origin.source_uses_refs() {
            assert!(
                self.snapshot
                    .source_inventory
                    .iter_uses()
                    .any(|used| used.id() == reference),
                "every contributing source use resolves within the selective snapshot"
            );
        }
    }
}

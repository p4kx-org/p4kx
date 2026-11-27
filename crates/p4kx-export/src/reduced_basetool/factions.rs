// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::record_identity::RecordIdentity;
use super::record_link::RecordLink;
use super::record_targets::RecordTargets;
use super::selected_texts::{SelectedTexts, TextRefs};
use crate::catalog_queries::factions::{FactionDefinition, FactionSnapshot, FriendlyFireOverride};
use crate::catalog_queries::{
    CatalogRecordSelection, FactionText, ParsedCatalogQueries, PublicNameRef,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::source_value_catalog::SourceRecordReference;

pub(super) const PATH: &str = "definitions.factions";

pub(super) struct Factions {
    pub(super) table: StagedCatalogTable,
    pub(super) targets: RecordTargets,
}

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    reputations: &RecordTargets,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<Factions, ReducedError> {
    parsed.with_factions(CatalogRecordSelection::All, cancel, |snapshot| {
        stage(&snapshot, reputations, scratch, cancel, texts)
    })?
}

fn stage(
    snapshot: &FactionSnapshot<'_>,
    reputations: &RecordTargets,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<Factions, ReducedError> {
    completeness::inventory(PATH, snapshot.availability(), snapshot.coverage())?;
    let mut targets = RecordTargets::default();
    for definition in snapshot.rows() {
        checkpoint(cancel)?;
        let header = definition.source();
        completeness::value(PATH, &header.guid())?;
        completeness::value(PATH, header.record_index())?;
        completeness::value(PATH, header.class_name())?;
        completeness::value(PATH, header.record_path())?;
        targets.insert(PATH, header)?;
    }
    let mut output = StagedCatalogRows::new(scratch, cancel);
    let mut references = TextRefs::default();
    for definition in snapshot.rows() {
        checkpoint(cancel)?;
        output.append(&Faction::read(
            definition,
            &targets,
            reputations,
            &mut references,
        )?)?;
    }
    let table = output.finish(snapshot.availability())?;
    completeness::rows(PATH, snapshot.rows().len(), table.len())?;
    texts.add(
        references,
        snapshot.public_names(),
        snapshot.game_texts(),
        cancel,
    )?;
    Ok(Factions { table, targets })
}

type LinkCell<'read> = Cell<RecordLink<'read>>;
type LinkList<'read> = Cell<Vec<LinkCell<'read>>>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Faction<'read, 'source> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    name: Cell<&'read FactionText<'source, PublicNameRef>>,
    #[serde(rename = "factionType")]
    kind: Cell<&'read str>,
    default_reaction: Cell<&'read str>,
    able_to_arrest: Cell<&'read bool>,
    polices_lawful_trespass: Cell<&'read bool>,
    polices_criminality: Cell<&'read bool>,
    no_legal_rights: Cell<&'read bool>,
    #[serde(rename = "alliedFactions")]
    allies: LinkList<'read>,
    #[serde(rename = "enemyFactions")]
    enemies: LinkList<'read>,
    friendly_fire_behavior_overrides: Cell<Vec<Cell<FriendlyFireRule<'read>>>>,
    #[serde(rename = "factionReputationRef")]
    reputation: LinkCell<'read>,
}

impl<'read, 'source> Faction<'read, 'source> {
    fn read(
        definition: &'read FactionDefinition<'source>,
        factions: &RecordTargets,
        reputations: &RecordTargets,
        references: &mut TextRefs<'read>,
    ) -> Result<Self, ReducedError> {
        let name = completeness::cell(PATH, definition.name())?;
        if let Some(FactionText::Localized { reference }) = name.value {
            references.name(Some(reference));
        }
        Ok(Self {
            identity: definition.source().into(),
            name,
            kind: completeness::cell(PATH, definition.faction_type())?.map(|label| *label),
            default_reaction: completeness::cell(PATH, definition.default_reaction())?
                .map(|label| *label),
            able_to_arrest: completeness::cell(PATH, definition.able_to_arrest())?,
            polices_lawful_trespass: completeness::cell(
                PATH,
                definition.polices_lawful_trespass(),
            )?,
            polices_criminality: completeness::cell(PATH, definition.polices_criminality())?,
            no_legal_rights: completeness::cell(PATH, definition.no_legal_rights())?,
            allies: faction_links(definition.allied_factions(), factions)?,
            enemies: faction_links(definition.enemy_factions(), factions)?,
            friendly_fire_behavior_overrides: completeness::list(
                PATH,
                definition.friendly_fire_behavior_overrides(),
                FriendlyFireRule::read,
            )?,
            reputation: completeness::cell(PATH, definition.faction_reputation_ref())?
                .try_map(|link| RecordLink::read_source(PATH, link, reputations, |_| true))?,
        })
    }
}

fn faction_links<'read>(
    source: &'read SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'_>>>>>,
    factions: &RecordTargets,
) -> Result<LinkList<'read>, ReducedError> {
    completeness::list(PATH, source, |link| {
        RecordLink::read_source(PATH, link, factions, |_| true)
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FriendlyFireRule<'read> {
    source_type: Cell<&'read str>,
    reaction_type: Cell<&'read str>,
    should_allow_friendly_fire: Cell<&'read bool>,
}

impl<'read> FriendlyFireRule<'read> {
    fn read(rule: &'read FriendlyFireOverride<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            source_type: completeness::cell(PATH, rule.source_type())?.map(|name| *name),
            reaction_type: completeness::cell(PATH, rule.reaction_type())?.map(|name| *name),
            should_allow_friendly_fire: completeness::cell(
                PATH,
                rule.should_allow_friendly_fire(),
            )?,
        })
    }
}

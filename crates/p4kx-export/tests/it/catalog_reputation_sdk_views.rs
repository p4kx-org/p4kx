// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    BlackboardValue, CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
    FactionReputationDefinition, FactionReputationPresentation, FactionText, GameTextRef,
    ReputationRelation, ReputationSelection, ReputationSnapshot, ResourceNumber, SourcePayload,
};
use p4kx_extraction::{Language, SourceEncryptionState, SourceUseRole, SourcedValue};

use super::TestResult;
use crate::commodity_v3_fixture::{self as archive_fixture, DcbOrigin};

use crate::catalog_reputation::owner_fixture as fixture;
#[path = "catalog_reputation_sdk_progression.rs"]
mod progression;
pub(super) use progression::check_scope_progression;

#[test]
fn reputation_sdk_nested_views_keep_original_values_text_domains_and_unknown_encryption()
-> TestResult {
    for (origin, marker) in [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        for literal in [false, true] {
            let (_folder, archive) = archive_fixture::open(&fixture::owner(literal), origin)?;
            let session = CatalogQuerySession::open(
                &archive,
                CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
                None,
            )?;
            let mut request = ReputationSelection::default();
            request.faction_reputations = Some(CatalogRecordSelection::Guid(
                archive_fixture::guid(fixture::MAIN).parse()?,
            ));
            session.with_reputation(request, None, |snapshot| {
                check_owner(&snapshot, literal, marker)
            })??;
        }
    }
    Ok(())
}

fn check_owner(
    snapshot: &ReputationSnapshot<'_>,
    literal: bool,
    marker: Option<bool>,
) -> TestResult {
    let rows = &snapshot
        .faction_reputations
        .as_ref()
        .ok_or("selected faction family")?
        .rows;
    let [owner] = rows.as_slice() else {
        return Err("one selected physical faction reputation".into());
    };
    let owner: &FactionReputationDefinition<'_> = owner;
    let presentation: &FactionReputationPresentation<'_> = owner.presentation();
    assert_eq!(
        presentation.geid().value().map(|value| value.get()),
        Some(fixture::GEID),
        "the full unsigned identifier is not narrowed"
    );
    assert_eq!(
        presentation.is_npc().value(),
        Some(&true),
        "the actual source flag remains present"
    );
    check_origin(snapshot, presentation.geid(), marker)?;
    check_origin(snapshot, presentation.background(), marker)?;
    check_description(snapshot, presentation, literal, marker)?;
    let hostility = owner
        .relations()
        .hostility_params()
        .value()
        .ok_or("owned hostile rule")?;
    check_relation(snapshot, hostility, marker)?;
    check_properties(snapshot, owner, marker)?;
    assert!(
        snapshot.public_names.is_empty(),
        "protected reputation texts do not move to the public-name domain"
    );
    for root in owner.authored_fields().roots() {
        assert!(
            owner.authored_fields().value(root.value()).is_some(),
            "all additional owner field links remain in their original arena"
        );
    }
    Ok(())
}

fn check_properties(
    snapshot: &ReputationSnapshot<'_>,
    owner: &FactionReputationDefinition<'_>,
    marker: Option<bool>,
) -> TestResult {
    let properties = owner
        .properties_bb()
        .value()
        .ok_or("original blackboard slots")?;
    let float = properties
        .iter()
        .filter_map(|row| row.value())
        .find(|row| row.name().value() == Some(&"fixtureFloat"))
        .ok_or("original floating property")?;
    let body = float
        .dynamic_property()
        .value()
        .ok_or("owned dynamic property")?;
    assert!(
        matches!(body.value(), BlackboardValue::Float { value } if value.value().is_some_and(|number| number.raw_bits() == fixture::FLOAT_BITS)),
        "the dynamic Single retains negative-zero bits"
    );
    let unknown = properties
        .iter()
        .filter_map(|row| row.value())
        .find(|row| row.name().value() == Some(&"fixtureUnknown"))
        .ok_or("original unknown property")?;
    check_unknown(
        snapshot,
        unknown
            .dynamic_property()
            .value()
            .ok_or("unknown dynamic body")?,
        marker,
    )?;
    Ok(())
}

fn check_description(
    snapshot: &ReputationSnapshot<'_>,
    presentation: &FactionReputationPresentation<'_>,
    literal: bool,
    marker: Option<bool>,
) -> TestResult {
    check_origin(snapshot, presentation.description(), marker)?;
    match presentation.description().value() {
        Some(FactionText::Literal { text }) if literal => {
            assert_eq!(
                *text,
                fixture::LITERAL,
                "a key-looking authored String remains literal"
            );
            assert!(
                snapshot
                    .game_texts
                    .iter()
                    .all(|row| row.key().value() != Some(&fixture::LITERAL)),
                "literal content creates no localization lookup"
            );
        }
        Some(FactionText::Localized { reference }) if !literal => {
            check_locales(snapshot, reference, marker)?;
        }
        _ => return Err("the source string/locale distinction is retained".into()),
    }
    Ok(())
}

fn check_relation(
    snapshot: &ReputationSnapshot<'_>,
    relation: &ReputationRelation<'_>,
    marker: Option<bool>,
) -> TestResult {
    let scope = relation.scope().value().ok_or("original scope link")?;
    assert_eq!(
        scope.target_type().value(),
        Some(&"SReputationScopeParams"),
        "the retained target keeps its original declared family"
    );
    check_origin(snapshot, relation.scope(), marker)?;
    let relation_marker = relation
        .marker_params()
        .value()
        .ok_or("owned relationship marker")?;
    let Some(FactionText::Localized { reference }) = relation_marker.description().value() else {
        return Err("the original marker description is a protected locale".into());
    };
    check_locales(snapshot, reference, marker)?;
    check_origin(snapshot, relation_marker.icon(), marker)?;
    for root in relation_marker.authored_fields().roots() {
        assert!(
            relation_marker
                .authored_fields()
                .value(root.value())
                .is_some(),
            "marker fields resolve in the marker's own arena"
        );
    }
    Ok(())
}

fn check_unknown(
    snapshot: &ReputationSnapshot<'_>,
    body: &p4kx_export::catalog_queries::BlackboardPropertyBody<'_>,
    marker: Option<bool>,
) -> TestResult {
    assert!(
        matches!(body.value(), BlackboardValue::Authored),
        "an unclassified dynamic class is not guessed as a known primitive"
    );
    let arena = body.authored_fields();
    let root = arena
        .roots()
        .iter()
        .find(|field| field.name().value() == Some(&"value"))
        .ok_or("original unknown field")?;
    let node = arena
        .value(root.value())
        .ok_or("the original owner-local field target")?;
    assert!(
        matches!(node.value().value(), Some(SourcePayload::Number { value: ResourceNumber::Single(value) }) if value.raw_bits() == fixture::RAW_BITS),
        "the unclassified field retains its exact original NaN bits"
    );
    check_origin(snapshot, node.value(), marker)
}

fn check_locales(
    snapshot: &ReputationSnapshot<'_>,
    reference: &GameTextRef,
    marker: Option<bool>,
) -> TestResult {
    let text = snapshot
        .game_text(reference)
        .ok_or("same-domain protected text target")?;
    assert_eq!(
        text.key().value(),
        Some(&"@own_description"),
        "the original protected key is retained"
    );
    let translations = text.translations().value().ok_or("selected translations")?;
    for (language, encrypted) in [("en", Some(true)), ("de", marker)] {
        let forms = translations
            .get(language)
            .ok_or("the requested original locale")?;
        check_origin(snapshot, &forms.source_text, encrypted)?;
        check_origin(snapshot, &forms.display_text, encrypted)?;
    }
    Ok(())
}

pub(super) fn check_origin<T>(
    snapshot: &ReputationSnapshot<'_>,
    cell: &SourcedValue<T>,
    marker: Option<bool>,
) -> TestResult {
    assert_eq!(
        cell.origin().contains_encrypted_source(),
        marker,
        "the cell keeps its own actual input encryption state"
    );
    for reference in cell.origin().source_uses_refs() {
        let used = snapshot
            .source_inventory
            .iter_uses()
            .find(|row| row.id() == reference)
            .ok_or("the originating query's retained use")?;
        assert!(
            snapshot
                .source_inventory
                .iter_sources()
                .any(|row| row.id() == used.source()),
            "every contributing use resolves to the same snapshot's physical source"
        );
    }
    Ok(())
}

pub(super) fn check_mixed_translation<T>(
    snapshot: &ReputationSnapshot<'_>,
    cell: &SourcedValue<T>,
) -> TestResult {
    let mut plain_payload = false;
    let mut encrypted_selector = false;
    for reference in cell.origin().source_uses_refs() {
        let used = snapshot
            .source_inventory
            .iter_uses()
            .find(|row| row.id() == reference)
            .ok_or("the actual mixed translation input")?;
        plain_payload |= used.role() == SourceUseRole::Payload
            && used.effective_encryption() == SourceEncryptionState::Unencrypted;
        encrypted_selector |= used.role() == SourceUseRole::Selector
            && used.effective_encryption() == SourceEncryptionState::Encrypted;
    }
    assert!(
        plain_payload && encrypted_selector,
        "the plain locale payload and encrypted authored selector remain distinct"
    );
    Ok(())
}

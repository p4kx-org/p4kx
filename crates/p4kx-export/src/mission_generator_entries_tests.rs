// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::read::Lookup;
use super::{Grants, Requirements};
use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::OriginalFlowFields;
use crate::guid_text::CatalogGuid;
use crate::mission_generator::entries as mission_generator_entries;
use crate::record_fields::identity;
use p4kx_extraction::{Guid, Record, SourceInventory, SourceLocation, SourcedValue};

#[path = "mission_generator_entries_fixture.rs"]
mod fixture;

#[derive(Default)]
struct Survey {
    lookup: Option<Lookup>,
    candidates: Vec<(u32, String, Option<Guid>)>,
    skipped: usize,
}

#[test]
fn reverse_candidate_survey_retains_outside_intro_and_duplicate_sources() -> TestResult {
    test_support::with_input(&fixture::database(false, false), |forge, input| {
        let mut grants = Grants::new(input.origins());
        let mut survey = Survey::default();
        for record in forge.records().filter(|record| {
            record
                .type_name()
                .is_ok_and(|kind| kind == "ContractGenerator")
        }) {
            survey_record(&record, input, &mut grants, &mut survey)?;
        }
        assert_eq!(
            survey.skipped, 3,
            "development entries and missing identities follow canonical admission"
        );
        assert_eq!(
            survey.candidates.len(),
            4,
            "the parent, valid sub-contract, collision and intro remain"
        );
        let owners: Vec<_> = survey
            .candidates
            .iter()
            .filter(|(_, guid, sub)| guid == fixture::CONTRACT_TEXT && sub.is_none())
            .map(|(index, _, _)| *index)
            .collect();
        assert_eq!(
            owners,
            [1, 2],
            "the survey sees an outside logical-ID contender"
        );
        let mut resolver = grants.resolver();
        let result = resolver.grants(
            &survey
                .lookup
                .ok_or("the selected root's original requirement")?,
        )?;
        let [intro] = result.value().ok_or("an observed grant list")?.as_slice() else {
            return Err("the successful outside intro grant".into());
        };
        assert_eq!(
            intro.value().map(|guid| guid.0.to_string()),
            Some(fixture::INTRO_TEXT.to_owned()),
            "a narrow root's required intro can be supplied by another Generator"
        );
        verify_outside_origin(intro, &input.origins().uses.freeze()?);
        Ok(())
    })
}

fn survey_record<'source>(
    record: &Record<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    grants: &mut Grants,
    survey: &mut Survey,
) -> TestResult {
    let tree = record.tree()?;
    let scope = input.record_scope(record, &tree)?;
    let fields = OriginalFlowFields::read(&scope, None)?;
    let context = input.record_context(record)?;
    let mut current = Grants::new(input.origins());
    survey.skipped += mission_generator_entries::handlers(
        &tree,
        &fields,
        &context,
        &mut current,
        |handler, current| {
            mission_generator_entries::candidates(&tree, handler, &fields, current, |candidate| {
                let requirements = Requirements::shared(&fields, handler, candidate.contract)?;
                mission_generator_entries::rows(&tree, candidate.contract, |sub| {
                    let id = identity(candidate.contract).ok_or("admitted contract identity")?;
                    survey.candidates.push((
                        record.index(),
                        id.to_string(),
                        sub.and_then(identity),
                    ));
                    if survey.lookup.is_none() && record.index() == 1 && sub.is_none() {
                        let source = requirements.row(&fields, sub)?;
                        survey.lookup = source
                            .value()
                            .and_then(|items| items.first())
                            .map(|required| required.lookup.clone());
                    }
                    Ok(())
                })
            })
        },
    )
    .map_err(|error| -> Box<dyn std::error::Error> { error })?;
    grants.merge(current)?;
    Ok(())
}

fn verify_outside_origin(intro: &SourcedValue<CatalogGuid>, inventory: &SourceInventory) {
    assert!(
        intro.origin().source_uses_refs().iter().any(|reference| {
            inventory.iter_uses().any(|used| used.id() == reference &&
            matches!(used.location(), SourceLocation::Subresource { namespace, indices, .. }
                if namespace == "dataforgeValue" && indices.first() == Some(&2)))
        }),
        "the grant retains evidence from its actual outside source owner"
    );
}

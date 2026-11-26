// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, InputScope, ProvenanceMode, SourceInput, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses, SourceValue, SourcedValue,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde::Serialize;

use super::{DefinitionFacts, FieldFact, RecordPortTypes};
use crate::build::original_input::test_support::TestResult;

#[test]
fn record_port_attributes_match_flat_proofs_for_each_occurrence() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", "definition"))
        .entry(TestEntry::stored("Data\\Loadout.xml", "occurrence").encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let [definition, occurrence] = archive.entries() else {
        return Err("two physical Record-port fixture entries".into());
    };
    for mode in [ProvenanceMode::Full, ProvenanceMode::MarkersOnly] {
        let sources = SourceUses::with_provenance_mode(mode);
        let definition = archive
            .entry_origin(definition)
            .ok_or("definition origin")?;
        let occurrence = archive
            .entry_origin(occurrence)
            .ok_or("occurrence origin")?;
        let clear = sources.observe(SourceInput::Archive(&definition), InputScope::Payload)?;
        let encrypted = sources.observe(SourceInput::Archive(&occurrence), InputScope::Payload)?;
        let foreign = SourceUses::with_provenance_mode(mode);
        let foreign_scope =
            foreign.observe(SourceInput::Archive(&definition), InputScope::Payload)?;
        compare_occurrences(&sources, &clear, &encrypted, &foreign_scope)?;
    }
    Ok(())
}

fn compare_occurrences(
    sources: &SourceUses,
    clear: &SourceScope,
    encrypted: &SourceScope,
    foreign_scope: &SourceScope,
) -> TestResult {
    let facts = definition_facts(clear)?;
    let duplicate = facts.selectors.first().ok_or("definition selector")?;
    let cases = [
        (
            vec![
                observe(clear, "firstName", SourceUseRole::Selector)?,
                duplicate.clone(),
            ],
            true,
            Some(false),
        ),
        (
            vec![observe(encrypted, "secondName", SourceUseRole::Selector)?],
            true,
            Some(true),
        ),
        (Vec::new(), false, None),
        (
            vec![observe(
                foreign_scope,
                "portSelector",
                SourceUseRole::Selector,
            )?],
            true,
            None,
        ),
    ];
    for (selectors, complete, marker) in cases {
        let actual = facts.bind(sources, selectors.clone(), complete)?;
        let definition_and_occurrence = facts
            .selectors
            .iter()
            .chain(selectors.iter())
            .cloned()
            .collect::<Vec<_>>();
        let expected = FlatProof {
            sources,
            selectors: &definition_and_occurrence,
            complete,
        };
        expected.check(&facts.min, &actual.min_size)?;
        expected.check(&facts.max, &actual.max_size)?;
        expected.check(&facts.flags, &actual.flags)?;
        expected.check(&facts.port_tags, &actual.port_tags)?;
        expected.check(&facts.required_tags, &actual.required_tags)?;
        expected.check(&facts.display_name_key, &actual.display_name_key)?;
        assert_eq!(
            actual.min_size.origin().contains_encrypted_source(),
            marker,
            "the original occurrence can independently change the encryption marker"
        );
    }
    Ok(())
}

fn observe(scope: &SourceScope, field: &str, role: SourceUseRole) -> TestResult<SourceUse> {
    Ok(scope.at(
        SourceLocation::Metadata {
            field: field.into(),
        },
        role,
    )?)
}

fn definition_facts(clear: &SourceScope) -> TestResult<DefinitionFacts> {
    let selector = observe(clear, "portSelector", SourceUseRole::Selector)?;
    let declaration = observe(clear, "declaration", SourceUseRole::Selector)?;
    Ok(DefinitionFacts {
        selectors: vec![selector.clone(), selector],
        types: RecordPortTypes::test_absent(declaration.clone()),
        min: FieldFact {
            value: SourceValue::Present(2),
            inputs: vec![observe(clear, "minSize", SourceUseRole::Conversion)?],
            complete: true,
        },
        max: FieldFact {
            value: SourceValue::Absent,
            inputs: vec![declaration.clone()],
            complete: true,
        },
        flags: FieldFact {
            value: SourceValue::Invalid,
            inputs: vec![observe(clear, "flags", SourceUseRole::Conversion)?],
            complete: true,
        },
        port_tags: FieldFact {
            value: SourceValue::Null,
            inputs: vec![declaration.clone()],
            complete: true,
        },
        required_tags: FieldFact {
            value: SourceValue::Unreadable,
            inputs: vec![observe(clear, "requiredTags", SourceUseRole::Attempted)?],
            complete: false,
        },
        display_name_key: FieldFact {
            value: SourceValue::Present("@testPort".to_owned()),
            inputs: vec![
                observe(clear, "nameKey", SourceUseRole::Payload)?,
                declaration,
            ],
            complete: true,
        },
    })
}

struct FlatProof<'source> {
    sources: &'source SourceUses,
    selectors: &'source [SourceUse],
    complete: bool,
}

impl FlatProof<'_> {
    fn check<T: Clone + Serialize>(
        &self,
        fact: &FieldFact<T>,
        actual: &SourcedValue<T>,
    ) -> TestResult {
        let mut inputs = fact.inputs.clone();
        inputs.extend_from_slice(self.selectors);
        let expected =
            self.sources
                .value(fact.value.clone(), &inputs, self.complete && fact.complete);
        assert_eq!(
            serde_json::to_vec(actual)?,
            serde_json::to_vec(&expected)?,
            "value state, marker and sorted references match the flat proof"
        );
        assert_eq!(
            actual.origin().source_set_complete(),
            expected.origin().source_set_complete(),
            "field-specific completeness survives the shared basis"
        );
        Ok(())
    }
}

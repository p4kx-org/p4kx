// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::catalog_queries::{
    CatalogGuid, ContractEntry, ContractGeneratorDefinition, ContractSnapshot, DatasetAvailability,
    SourcePayload, SourceValueRef,
};
use p4kx_extraction::{
    Archive, Build, DataForge, DataForgeEntry, RecordTree, SourceValueState, StructValue, Value,
};

use super::evidence::Evidence;
use super::source::{self, Key, Rows};
use super::{Counts, TestResult, original, sets};

pub(super) fn owner(
    archive: &Archive,
    entry: &DataForgeEntry,
    tree: &RecordTree<'_>,
    sources: &Rows<'_, '_>,
    snapshot: &ContractSnapshot<'_>,
    counts: &mut Counts,
) -> TestResult {
    shared(archive, snapshot)?;
    let expected = sources.keys();
    let mut covered = BTreeSet::new();
    let evidence = Evidence::read(entry, tree, snapshot, sources)?;
    for row in &snapshot.rows {
        if row.base.source.record_index() != tree.record_index() {
            return Err("owner selection returned a foreign physical owner".into());
        }
        verify(tree, sources, snapshot, row, &evidence, counts)?;
        covered.insert(key(row)?);
        counts.owner_rows += 1;
    }
    for shadowed in &snapshot.coverage.shadowed_candidates {
        let key = (shadowed.contract_guid, shadowed.sub_contract_guid);
        if shadowed.selected_record_index != tree.record_index()
            || shadowed.winner_record_index == tree.record_index()
            || !expected.contains(&key)
        {
            return Err(
                "shadow coverage differs from the actual selected source identities".into(),
            );
        }
        covered.insert(key);
    }
    if covered != expected {
        return Err("an original logical row is neither returned nor explicitly shadowed".into());
    }
    Ok(())
}

pub(super) fn logical(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    expected: Key,
    snapshot: &ContractSnapshot<'_>,
    counts: &mut Counts,
) -> TestResult {
    shared(archive, snapshot)?;
    let [row] = snapshot.rows.as_slice() else {
        return Err("a source-selected logical identity needs exactly one canonical row".into());
    };
    if key(row)? != expected {
        return Err("logical lookup changed its requested parent or sub-contract identity".into());
    }
    let tree = original(forge, row.base.source.record_index())?;
    let sources = Rows::read(&tree)?;
    let evidence = Evidence::read(entry, &tree, snapshot, &sources)?;
    verify(&tree, &sources, snapshot, row, &evidence, counts)?;
    counts.logical_rows += 1;
    Ok(())
}

fn shared(archive: &Archive, snapshot: &ContractSnapshot<'_>) -> TestResult {
    let expected = archive.build().ok_or("installed Build identity")?;
    let actual = snapshot.build.as_ref().ok_or("SDK Build identity")?;
    if build_identity(expected) != build_identity(actual)
        || build_identity(expected).iter().any(|part| part.is_empty())
    {
        return Err("SDK and original complete Build identities differ".into());
    }
    let coverage = &snapshot.coverage;
    if !matches!(snapshot.availability, DatasetAvailability::Available)
        || !coverage.selected.unreadable_headers.is_empty()
        || !coverage.selected.unreadable_bodies.is_empty()
        || !coverage.projection_failures.is_empty()
        || !coverage.dependencies.unreadable_headers.is_empty()
        || !coverage.dependencies.survey_failures.is_empty()
        || !coverage.dependencies.forward_failures.is_empty()
    {
        return Err("the source comparison has an explicit SDK coverage gap".into());
    }
    if !snapshot.requested_languages.is_empty()
        || !snapshot.available_languages.is_empty()
        || !snapshot.localization_notes.is_empty()
        || snapshot.rows.len() != snapshot.mission_titles.len()
    {
        return Err("the no-locale query has unexpected text inputs or unpaired titles".into());
    }
    Ok(())
}

fn verify<'source>(
    tree: &RecordTree<'source>,
    sources: &Rows<'_, 'source>,
    snapshot: &ContractSnapshot<'_>,
    row: &ContractEntry<'_>,
    evidence: &Evidence<'_, 'source>,
    counts: &mut Counts,
) -> TestResult {
    if row.base.source.input_ref() != evidence.source_ref() {
        return Err("the physical occurrence belongs to a foreign Entry".into());
    }
    let site = sources
        .sites
        .get(evidence.site(&row.base.source)?)
        .ok_or("the original physical source site")?;
    if site.key != key(row)? {
        return Err("SDK identifiers differ from their original Guid values".into());
    }
    let contract = source::structure(Some(site.contract))?.ok_or("original contract Struct")?;
    let handler = source::structure(Some(site.handler))?.ok_or("original handler Struct")?;
    evidence.operand(
        contract.field("id").ok_or("original parent id")?,
        row.base.guid.origin(),
    )?;
    let sub = source::structure(site.sub)?;
    if let Some(sub) = sub {
        evidence.operand(
            sub.field("id").ok_or("original sub id")?,
            row.base.sub_contract_guid.origin(),
        )?;
    } else {
        if row.base.sub_contract_guid.state() != SourceValueState::Absent {
            return Err("the original parent row has no sub identifier".into());
        }
        evidence.operand(site.contract, row.base.sub_contract_guid.origin())?;
    }
    counts.identity_cells += 2;
    direct_values(contract, handler, row)?;
    let definition = definition(snapshot, tree)?;
    if definition.input_ref() != evidence.source_ref() {
        return Err("retained definition belongs to a different original Entry".into());
    }
    for (name, projected) in [
        ("template", row.base.template_guid),
        ("missionBrokerEntry", row.base.broker_entry_guid),
    ] {
        let expected = match contract.field(name) {
            Some(Value::Reference(value)) if value.location().is_none() => Some(value.guid()),
            _ => None,
        };
        if projected.map(CatalogGuid::get) != expected {
            return Err("SDK direct reference Guid differs from its original field".into());
        }
        reference(
            contract,
            name,
            definition,
            row.base.source.contract(),
            evidence,
            counts,
        )?;
    }
    sets::compare(tree, sources, site, row, snapshot, counts)
}

fn direct_values(
    contract: &StructValue<'_>,
    handler: &StructValue<'_>,
    row: &ContractEntry<'_>,
) -> TestResult {
    if row.base.debug_name != source::text(contract, "debugName")
        || row.base.handler != source::text(handler, "debugName")
        || row.base.not_for_release != source::boolean(contract, "notForRelease")
        || row.base.work_in_progress != source::boolean(contract, "workInProgress")
    {
        return Err("SDK direct authored values differ from the selected original contract".into());
    }
    Ok(())
}

fn key(row: &ContractEntry<'_>) -> TestResult<Key> {
    Ok((
        row.base
            .guid
            .value()
            .ok_or("a canonical contract needs its original id")?
            .get(),
        row.base
            .sub_contract_guid
            .value()
            .copied()
            .map(CatalogGuid::get),
    ))
}

fn definition<'read, 'source>(
    snapshot: &'read ContractSnapshot<'source>,
    tree: &RecordTree<'_>,
) -> TestResult<&'read ContractGeneratorDefinition<'source>> {
    let definition = snapshot
        .generators
        .iter()
        .find(|source| source.header().record_index().value() == Some(&tree.record_index()))
        .ok_or("the physical winner has no retained original definition")?;
    if definition.header().guid().value() != Some(&tree.guid())
        || definition.header().record_path().value().copied() != Some(tree.record_path())
    {
        return Err("retained original definition has a different physical identity".into());
    }
    Ok(definition)
}

fn reference<'source>(
    original: &StructValue<'source>,
    name: &str,
    definition: &ContractGeneratorDefinition<'_>,
    owner: SourceValueRef,
    proof: &Evidence<'_, 'source>,
    counts: &mut Counts,
) -> TestResult {
    let node = definition
        .source()
        .value(owner)
        .ok_or("original source owner link")?;
    let Some(SourcePayload::Struct { fields, .. }) = node.value().value() else {
        return Err("original arena owner is not a Struct".into());
    };
    let field = fields
        .iter()
        .find(|field| field.name().value().copied() == Some(name));
    let Some(original) = original.field(name) else {
        if field.is_some() {
            return Err("arena invented a source reference field".into());
        }
        return Ok(());
    };
    let field = field.ok_or("arena lost an original reference declaration")?;
    if field.value().value_index() != u64::try_from(proof.locations.ordinal(original)?)? {
        return Err("arena reference points to a different physical source slot".into());
    }
    let node = definition
        .source()
        .value(field.value())
        .ok_or("source field value")?;
    proof.operand(original, node.value().origin())?;
    if let Value::Reference(original) = original {
        let Some(SourcePayload::RecordReference { value }) = node.value().value() else {
            return Err("readable original reference was not retained as a typed reference".into());
        };
        if value.guid().value() != Some(&original.guid())
            || value.target_record_index().value() != Some(&original.record_index())
        {
            return Err(
                "arena reference differs from its original Guid or exact target Record index"
                    .into(),
            );
        }
        counts.reference_fields += 1;
    }
    Ok(())
}

fn build_identity(build: &Build) -> [&str; 4] {
    [
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    ]
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write as _};
use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch, Build, Exclusion};
use p4kx_media::{Digest, ExportOptions, ExportOutcome, Identity, MediaKind, MediaRecord, Status};
use serde_json::Value;

use super::animation_graph_accounting::clips;
use super::shader_accounting::require;
use super::{TestResult, manifests, privacy};

#[path = "animation_scope_sources.rs"]
mod sources;

#[path = "animation_scope_fixture.rs"]
mod fixture;

#[path = "animation_scope_loadout.rs"]
mod loadout;

#[path = "animation_scope_origins.rs"]
mod origins;

#[path = "animation_scope_adb.rs"]
mod adb;

#[path = "animation_scope_adb_rows.rs"]
mod adb_rows;

#[path = "animation_scope_dependencies.rs"]
mod dependencies;

#[path = "animation_scope_blend_wildcards.rs"]
mod blend_wildcards;

#[path = "animation_scope_contributors.rs"]
mod contributors;

#[path = "animation_scope_rows.rs"]
mod rows;

use self::sources::{Closure, Placement};

const MODEL_VAR: &str = "P4KX_ANIMATION_SCOPE_MODEL";
const ENTITY_VAR: &str = "P4KX_ANIMATION_SCOPE_ENTITY";
const RECEIPT: &str = "animation-definition-scope-receipt";

type RecordKey = (Option<usize>, String, Vec<u64>);

enum Selection {
    Model(String),
    Entity(String),
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and both P4KX_ANIMATION_SCOPE selectors"]
fn installed_animation_scopes_match_verified_source_and_placement_closures()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify_configured())
}

fn verify_configured() -> TestResult {
    let Some((model, entity)) = configured_selectors()? else {
        return Ok(());
    };
    let key = p4kx_testkit::real_archive_key()?;
    let archive = Archive::open_with_key(p4kx_testkit::real_archive_path()?, Some(key))?;
    let build = archive.build().ok_or("authoritative installed Build")?;
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    writeln!(
        io::stdout(),
        "verifying the installed model scope from original sources"
    )?;
    let model_sources = model.discover(&archive, &watch)?;
    writeln!(
        io::stdout(),
        "verifying the installed Entity scope from original sources"
    )?;
    let entity_sources = entity.discover(&archive, &watch)?;
    require(
        model_sources.has_events() || entity_sources.has_events(),
        "an authored physical event database in the verified scopes",
    )?;
    writeln!(io::stdout(), "checking the selected model publication")?;
    verify(&archive, build, &model, model_sources)?;
    writeln!(io::stdout(), "checking the selected Entity publication")?;
    verify(&archive, build, &entity, entity_sources)?;
    watch.check()?;
    let directory = archive
        .path()
        .parent()
        .ok_or("installed channel directory")?;
    require(
        Build::read_from(directory)?.as_ref() == Some(build),
        "unchanged Build",
    )?;
    writeln!(
        io::stdout(),
        "animation scope witness Build {} {} {} {}",
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    )?;
    Ok(())
}

fn configured_selectors() -> TestResult<Option<(Selection, Selection)>> {
    let (model, entity) = match (env::var(MODEL_VAR), env::var(ENTITY_VAR)) {
        (Ok(model), Ok(entity)) => (model, entity),
        (Err(env::VarError::NotPresent), Err(env::VarError::NotPresent)) => {
            writeln!(
                io::stdout(),
                "installed animation scope witness skipped: selectors unspecified"
            )?;
            return Ok(None);
        }
        _ => return Err("both installed animation scope selectors are required".into()),
    };
    require(
        !entity.trim().is_empty()
            && !model.contains(['*', '?', ':', '!', '\0'])
            && !model
                .split(['/', '\\'])
                .any(|part| matches!(part, "" | "." | ".."))
            && model.rsplit_once('.').is_some_and(|(stem, extension)| {
                !stem.is_empty()
                    && (extension.eq_ignore_ascii_case("cdf")
                        || extension.eq_ignore_ascii_case("chr"))
            }),
        "one exact CDF/CHR and one Entity selector are required",
    )?;
    Ok(Some((Selection::Model(model), Selection::Entity(entity))))
}

impl Selection {
    fn discover<'archive>(
        &self,
        archive: &'archive Archive,
        watch: &ArchiveWatch,
    ) -> TestResult<Closure<'archive>> {
        match self {
            Self::Model(path) => Closure::model(archive, watch, path),
            Self::Entity(selector) => Closure::entity(archive, watch, selector),
        }
    }

    fn options(&self) -> TestResult<ExportOptions> {
        let mut options = ExportOptions {
            kinds: vec![MediaKind::AnimationDefinition],
            exclude: Exclusion::parse(["*.pak", "*.socpak"])?,
            ..ExportOptions::default()
        };
        match self {
            Self::Model(path) => options.animation_model = Some(path.clone()),
            Self::Entity(selector) => options.animation_entity = Some(selector.clone()),
        }
        options.limits.output_bytes = manifests::MAX_ARTIFACT_BYTES;
        Ok(options)
    }
}

fn verify(
    archive: &Archive,
    build: &Build,
    selection: &Selection,
    mut expected: Closure<'_>,
) -> TestResult<BTreeMap<usize, usize>> {
    let output = tempfile::tempdir()?;
    let ExportOutcome::Published { index, .. } =
        p4kx_media::export(archive, output.path(), &selection.options()?)?
    else {
        return Err("expected a complete scoped definition generation".into());
    };
    verify_build(&index, build)?;
    require(
        index.source_entries.written == expected.documents.len() as u64,
        "only the independently selected physical documents were written",
    )?;
    let kind = index
        .kinds
        .iter()
        .find(|kind| kind.provider.kind == MediaKind::AnimationDefinition)
        .ok_or("definition stream")?;
    let root = output.path().join(&index.artifact_base);
    let stream = BufReader::new(File::open(root.join(&kind.artifact.path))?);
    let blend_matches = observe_rows(archive, &root, &index.source.digest, stream, &mut expected)?;
    expected.verify()?;
    output.close()?;
    expected.report()?;
    Ok(blend_matches)
}

fn observe_rows(
    archive: &Archive,
    root: &Path,
    digest: &Digest,
    stream: impl BufRead,
    expected: &mut Closure<'_>,
) -> TestResult<BTreeMap<usize, usize>> {
    let mut placements = 0;
    let mut databases = expected.databases.iter().enumerate();
    let mut identities = BTreeSet::new();
    let mut fields = origins::Fields::default();
    let mut joins = contributors::Joins::default();
    for (stream_ordinal, line) in stream.lines().enumerate() {
        let row: MediaRecord = serde_json::from_str(&line?)?;
        require(row.source.root == *digest, "one original archive root")?;
        require(row.status == Status::Complete, "complete scoped source row")?;
        require(
            identities.insert(record_key(&row)?),
            "unique source-local record identity",
        )?;
        if row.subresource.namespace == RECEIPT {
            let placement = expected
                .placements
                .get(placements)
                .ok_or("extra placement receipt")?;
            receipt(&row, placement, placements, expected.input_entry)?;
            expect_origins(&mut fields, stream_ordinal, placement);
            placements += 1;
        } else if row.subresource.namespace == adb::BINDING {
            let (ordinal, binding) = databases.next().ok_or("extra Entity database binding")?;
            binding.verify(&row, ordinal)?;
            fields.expect(
                stream_ordinal,
                "/data/payload/authored_path",
                &binding.sites,
            );
        } else if !expected.graph.observe(&row)?
            && row.subresource.namespace != contributors::NAMESPACE
        {
            rows::observe(root, &row, &mut expected.documents)?;
        }
        joins.observe(&row)?;
    }
    require(
        placements == expected.placements.len(),
        "every resolved placement retained",
    )?;
    require(
        databases.next().is_none(),
        "every authored Entity database binding retained",
    )?;
    let blend_matches = joins.verify(&expected.placements)?;
    fields.verify(root, archive)?;
    Ok(blend_matches)
}

fn record_key(row: &MediaRecord) -> TestResult<RecordKey> {
    require(
        row.kind == MediaKind::AnimationDefinition,
        "one animation-definition provider stream",
    )?;
    let entry = match &row.source.identity {
        Identity::Root => None,
        Identity::Entry { containers, entry } if containers.is_empty() => Some(entry.entry_index),
        _ => return Err("only root Archive definitions belong to this witness".into()),
    };
    Ok((
        entry,
        row.subresource.namespace.clone(),
        row.subresource.indices.clone(),
    ))
}

fn verify_build(index: &p4kx_media::Index, build: &Build) -> TestResult {
    require(index.complete, "complete scoped publication")?;
    let observed = index
        .source
        .build
        .as_ref()
        .ok_or("published Build receipt")?;
    require(
        observed.branch == build.branch()
            && observed.version == build.version()
            && observed.change_number == build.change_number()
            && observed.build_id == build.build_id(),
        "generation matches the independently opened Build",
    )
}

fn receipt(
    row: &MediaRecord,
    expected: &Placement,
    ordinal: usize,
    input_entry: usize,
) -> TestResult {
    require(
        matches!(row.source.identity, Identity::Root),
        "root directory scope receipt",
    )?;
    require(
        row.subresource.indices.as_slice() == [ordinal as u64],
        "original resolved placement occurrence",
    )?;
    let payload = row.data.get("payload").ok_or("placement payload")?;
    require(
        index_field(payload, "/input_entry_index") == Some(input_entry),
        "exact scope input Entry",
    )?;
    require(
        payload.pointer("/placement").and_then(Value::as_str) == Some(expected.role)
            && payload.get("placement_detail") == expected.detail.as_ref(),
        "placement role and original port or landing-gear coordinates",
    )?;
    require(
        payload
            .pointer("/owner_record_index")
            .and_then(Value::as_u64)
            == expected.owner.map(u64::from),
        "physical original owner Record",
    )?;
    require(
        payload.pointer("/outcome/state").and_then(Value::as_str) == Some(expected.state)
            && index_field(payload, "/outcome/entry_index") == expected.parameter,
        "independent character parameter scope disposition",
    )
}

fn expect_origins<'a>(fields: &mut origins::Fields<'a>, ordinal: usize, placement: &'a Placement) {
    let evidence = &placement.evidence;
    let names_field = match placement.role {
        "attachment" => "/data/payload/placement_detail/port_path",
        "landing-gear" => "/data/payload/placement_detail/owner_path",
        _ => return,
    };
    fields.expect(ordinal, names_field, &evidence.names);
    if placement.owner.is_some() {
        fields.expect(
            ordinal,
            "/data/payload/owner_record_index",
            &evidence.selection,
        );
    }
    if placement.parameter.is_some() {
        fields.expect(
            ordinal,
            "/data/payload/outcome/entry_index",
            &evidence.selection,
        );
    }
}

fn index_field(value: &Value, field: &str) -> Option<usize> {
    value.pointer(field)?.as_str()?.parse().ok()
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write as _};
use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch, Build, Exclusion};
use p4kx_media::{ExportOptions, ExportOutcome, Identity, MediaKind, MediaRecord, Status};
use serde_json::Value;

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

use self::sources::{Closure, Document, Placement};

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
) -> TestResult {
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
    let mut placements = 0;
    let mut identities = BTreeSet::new();
    let mut fields = origins::Fields::default();
    for (stream_ordinal, line) in stream.lines().enumerate() {
        let row: MediaRecord = serde_json::from_str(&line?)?;
        require(
            row.source.root == index.source.digest,
            "one original archive root",
        )?;
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
        } else {
            observe(&root, &row, &mut expected.documents)?;
        }
    }
    require(
        placements == expected.placements.len(),
        "every resolved placement retained",
    )?;
    expected.verify()?;
    fields.verify(&root, archive)?;
    output.close()?;
    expected.report()
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

fn observe(
    base: &Path,
    row: &MediaRecord,
    documents: &mut BTreeMap<usize, Document<'_>>,
) -> TestResult {
    let Identity::Entry { entry, .. } = &row.source.identity else {
        require(
            row.subresource.namespace == "animation-definition-event-binding",
            "recognized non-document animation row",
        )?;
        let payload = row.data.get("payload").ok_or("event binding payload")?;
        for (field, family) in [
            ("/parameter_entry_index", "chrparams"),
            ("/event_database_entry_index", "animevents"),
        ] {
            let index = index_field(payload, field).ok_or("event binding physical input")?;
            let input = documents
                .get(&index)
                .ok_or("binding source outside selected closure")?;
            require(
                input.family() == family,
                "binding has exact selected source roles",
            )?;
        }
        return Ok(());
    };
    let source = documents
        .get_mut(&entry.entry_index)
        .ok_or("unselected physical document")?;
    require(
        entry.path == source.entry.path(),
        "original physical Entry path",
    )?;
    source.actual.observe(&row.subresource.namespace);
    if row.subresource.namespace == "animation-definition-document" {
        require(
            row.data.pointer("/payload/family").and_then(Value::as_str) == Some(source.family()),
            "source extension and emitted definition family agree",
        )?;
        let raw = row
            .source
            .raw
            .as_ref()
            .ok_or("retained original definition")?;
        let decoded = manifests::check_payload(
            base,
            &raw.path,
            raw.size,
            &raw.digest,
            source.bytes.len() as u64,
        )?;
        require(
            decoded == source.bytes,
            "retained definition equals its verified original bytes",
        )?;
    }
    Ok(())
}

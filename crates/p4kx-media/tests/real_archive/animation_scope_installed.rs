// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::env;
use std::io::{self, Write as _};

use p4kx_extraction::{Archive, ArchiveWatch, Build};

use super::sources::Closure;
use super::{Selection, TestResult, require, verify};

const MODEL_VAR: &str = "P4KX_ANIMATION_SCOPE_MODEL";
const ENTITY_VAR: &str = "P4KX_ANIMATION_SCOPE_ENTITY";
const PREFLIGHT_VAR: &str = "P4KX_ANIMATION_SCOPE_PREFLIGHT";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Preflight,
    Publication,
}

impl Mode {
    fn configured() -> TestResult<Self> {
        match env::var(PREFLIGHT_VAR).as_deref() {
            Ok("1") => Ok(Self::Preflight),
            Err(env::VarError::NotPresent) => Ok(Self::Publication),
            _ => Err("set P4KX_ANIMATION_SCOPE_PREFLIGHT to 1 or unset it".into()),
        }
    }
}

pub(super) fn verify_configured() -> TestResult {
    let mode = Mode::configured()?;
    let Some((model, entity)) = configured_selectors(mode)? else {
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
    if mode == Mode::Preflight {
        report_scope("model", &model_sources)?;
        report_scope("Entity", &entity_sources)?;
        return verify_installed_build(&archive, &watch, build, "preflight");
    }
    require(
        model_sources.has_events() || entity_sources.has_events(),
        "an authored physical event database in the verified scopes",
    )?;
    writeln!(io::stdout(), "checking the selected model publication")?;
    let model = verify(&archive, build, &model, model_sources)?;
    writeln!(io::stdout(), "checking the selected Entity publication")?;
    let entity = verify(&archive, build, &entity, entity_sources)?;
    require(
        entity.database_matches > 0,
        "at least one verified Entity clip-contributor with an ADB binding",
    )?;
    writeln!(
        io::stdout(),
        "verified clip-contributors across placements: model {}, Entity {} ({} with an ADB binding)",
        model.total,
        entity.total,
        entity.database_matches,
    )?;
    verify_installed_build(&archive, &watch, build, "witness")
}

fn configured_selectors(mode: Mode) -> TestResult<Option<(Selection, Selection)>> {
    let (model, entity) = match (env::var(MODEL_VAR), env::var(ENTITY_VAR)) {
        (Ok(model), Ok(entity)) => (model, entity),
        (Err(env::VarError::NotPresent), Err(env::VarError::NotPresent))
            if mode == Mode::Publication =>
        {
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

fn report_scope(label: &str, source: &Closure<'_>) -> TestResult {
    let mut counts = BTreeMap::<&str, usize>::new();
    let mut bytes = 0;
    for document in source.documents.values() {
        *counts.entry(document.family()).or_default() += 1;
        bytes += document.bytes.len() as u64;
    }
    writeln!(
        io::stdout(),
        "animation scope preflight {label}: {} selected definition Entries {counts:?}, {} placements, {} authored database bindings, {bytes} verified decoded definition bytes, authored event timeline present: {}",
        source.documents.len(),
        source.placements.len(),
        source.databases.len(),
        source.has_events(),
    )?;
    Ok(())
}

fn verify_installed_build(
    archive: &Archive,
    watch: &ArchiveWatch,
    build: &Build,
    receipt: &str,
) -> TestResult {
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
        "animation scope {receipt} Build {} {} {} {}",
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    )?;
    Ok(())
}

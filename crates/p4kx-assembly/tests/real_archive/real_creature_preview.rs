// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Attachment, EntityNode, ForgePalettes, Geometry, LoadoutResolver,
    archive_entry_candidates,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, Build, DataForge, DataForgeEntry, Entry, MAX_DATAFORGE_SIZE,
};
use p4kx_testkit::{real_archive_key, real_archive_path};

use super::observe;

const ENTITY: &str = "P4KX_ANIMATION_SCOPE_ENTITY";
const PREFLIGHT: &str = "P4KX_ANIMATION_SCOPE_PREFLIGHT";
const MODEL_OUT: &str = "P4KX_ANIMATION_SCOPE_MODEL_OUT";
const READ_BUFFER_BYTES: usize = 8 << 10;

type WitnessResult<T> = Result<T, Box<dyn Error>>;

enum Mode {
    Preview,
    Preflight(PathBuf),
}

impl Mode {
    fn configured() -> WitnessResult<Self> {
        match env::var(PREFLIGHT) {
            Err(env::VarError::NotPresent) => Ok(Self::Preview),
            Ok(value) if value == "1" => {
                let output = PathBuf::from(env::var_os(MODEL_OUT).ok_or("model selector output")?);
                if !output.is_absolute() {
                    return Err("the private model selector output must be absolute".into());
                }
                Ok(Self::Preflight(output))
            }
            _ => Err("the preflight marker must be 1 when set".into()),
        }
    }
}

pub(super) fn verify() -> Result<(), String> {
    let mut stage = "reading the exact Entity selector and preflight configuration";
    run(&mut stage).map_err(|_| format!("selected creature witness stopped while {stage}"))
}

fn run(stage: &mut &'static str) -> WitnessResult<()> {
    let selector = env::var(ENTITY)?;
    if selector.trim().is_empty() {
        return Err("the exact Entity selector is empty".into());
    }
    let mode = Mode::configured()?;
    *stage = "opening the Archive and verifying its Build";
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let build = archive.build().ok_or("authoritative installed Build")?;
    let watch = ArchiveWatch::new(&archive);
    unchanged(&archive, build, &watch)?;
    if archive
        .issues()
        .iter()
        .any(|(_, issue)| issue.is_archive_wide())
    {
        return Err("complete Archive directory required".into());
    }
    *stage = "reading the CRC-verified DataForge Entry";
    let bytes = dataforge_bytes(&archive, &watch)?;
    let forge = DataForge::open(&bytes)?;
    *stage = "resolving the exact creature Entity";
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge)
        .with_files(&files)
        .stopping_at_docked_vehicles();
    let record = resolver.entities().find(&selector)?;
    let tree = resolver.resolve(&record)?;
    let geometry = tree
        .geometry()
        .filter(|geometry| geometry.is_rigged_file_outside_human_rigs())
        .ok_or("a root creature rig")?;
    if tree.animation_database().is_none() {
        return Err("the creature has no authored animation database".into());
    }
    unchanged(&archive, build, &watch)?;
    match mode {
        Mode::Preflight(output) => {
            *stage = "verifying the creature root model and saving its private selector";
            preflight(&archive, build, &watch, &tree, geometry, &output)
        }
        Mode::Preview => {
            *stage = "reading, fitting and checking the in-memory creature preview";
            preview(&archive, build, &watch, &tree, &forge)
        }
    }
}

fn preflight(
    archive: &Archive,
    build: &Build,
    watch: &ArchiveWatch,
    tree: &EntityNode,
    geometry: &Geometry,
    output: &Path,
) -> WitnessResult<()> {
    let mut complete = tree.issues().is_empty();
    tree.visit_ports(|_, route| {
        complete &= match route.last().map(|port| port.attachment()) {
            Some(Attachment::Entity(child)) => child.issues().is_empty(),
            Some(Attachment::Unresolved(_)) => false,
            _ => true,
        };
    });
    if !complete {
        return Err("the selected creature has unresolved Loadout inputs".into());
    }
    let entry = model_entry(archive, geometry, watch)?;
    if !is_model_selector(entry.path()) {
        return Err("the verified root model is not an exact CDF or CHR selector".into());
    }
    copy_verified(archive, entry, watch, &mut io::sink())?;
    unchanged(archive, build, watch)?;
    writeln!(File::create_new(output)?, "{}", entry.path())?;
    println!("selected creature preflight verified one root model and its Build");
    Ok(())
}

fn preview(
    archive: &Archive,
    build: &Build,
    watch: &ArchiveWatch,
    tree: &EntityNode,
    forge: &DataForge<'_>,
) -> WitnessResult<()> {
    let models = ArchiveModels::new(archive);
    let result = observe(tree, &models, ForgePalettes::new(forge))?;
    if result.read == 0 || result.fitted == 0 || result.written == 0 {
        return Err("the creature preview must read, fit and write positive clip counts".into());
    }
    unchanged(archive, build, watch)?;
    println!(
        "selected creature preview Build {} {} {} {}: clips read {}, fitted {}, written {}, GLB bytes {}, unavailable reports {}",
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
        result.read,
        result.fitted,
        result.written,
        result.bytes,
        result.unavailable.len(),
    );
    Ok(())
}

fn dataforge_bytes(archive: &Archive, watch: &ArchiveWatch) -> WitnessResult<Vec<u8>> {
    let entry = DataForgeEntry::locate(archive).ok_or("one original DataForge Entry")?;
    let entry =
        unique_entry(archive, entry.path(), watch)?.ok_or("one original DataForge Entry")?;
    let size = entry.uncompressed_size();
    if size > MAX_DATAFORGE_SIZE {
        return Err("DataForge input exceeds its existing read allowance".into());
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(usize::try_from(size)?)?;
    copy_verified(archive, entry, watch, &mut bytes)?;
    Ok(bytes)
}

fn model_entry<'archive>(
    archive: &'archive Archive,
    geometry: &Geometry,
    watch: &ArchiveWatch,
) -> WitnessResult<&'archive Entry> {
    for candidate in archive_entry_candidates(geometry.path()) {
        if let Some(entry) = unique_entry(archive, &candidate, watch)? {
            return Ok(entry);
        }
    }
    Err("the authored root model does not identify an Archive Entry".into())
}

fn unique_entry<'archive>(
    archive: &'archive Archive,
    path: &str,
    watch: &ArchiveWatch,
) -> WitnessResult<Option<&'archive Entry>> {
    let mut found = None;
    for entry in archive.entries() {
        watch.poll()?;
        if same_path(entry.path(), path)
            && (archive.raw_path(entry.index()).is_some() || found.replace(entry).is_some())
        {
            return Err("the requested Archive path is ambiguous or repaired".into());
        }
    }
    Ok(found)
}

fn copy_verified(
    archive: &Archive,
    entry: &Entry,
    watch: &ArchiveWatch,
    output: &mut impl Write,
) -> WitnessResult<()> {
    let mut reader = archive.open_entry_verified(entry)?;
    let mut buffer = [0; READ_BUFFER_BYTES];
    let mut remaining = entry.uncompressed_size();
    loop {
        watch.poll()?;
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        remaining = remaining
            .checked_sub(count as u64)
            .ok_or("Archive Entry size")?;
        output.write_all(buffer.get(..count).ok_or("Archive read buffer")?)?;
    }
    if remaining != 0 {
        return Err("the verified Archive Entry disagrees with its declared size".into());
    }
    Ok(())
}

fn same_path(left: &str, right: &str) -> bool {
    left.len() == right.len()
        && left.bytes().zip(right.bytes()).all(|(left, right)| {
            left.eq_ignore_ascii_case(&right)
                || (matches!(left, b'/' | b'\\') && matches!(right, b'/' | b'\\'))
        })
}

fn is_model_selector(path: &str) -> bool {
    !path.contains(['*', '?', ':', '!', '\0', '\r', '\n'])
        && !path
            .split(['/', '\\'])
            .any(|part| matches!(part, "" | "." | ".."))
        && path.rsplit_once('.').is_some_and(|(stem, extension)| {
            !stem.is_empty()
                && (extension.eq_ignore_ascii_case("cdf") || extension.eq_ignore_ascii_case("chr"))
        })
}

fn unchanged(archive: &Archive, build: &Build, watch: &ArchiveWatch) -> WitnessResult<()> {
    watch.check()?;
    let channel = archive.path().parent().ok_or("installed channel folder")?;
    if Build::read_from(channel)?.as_ref() != Some(build) {
        return Err("the installed Build changed while verifying the creature".into());
    }
    Ok(())
}

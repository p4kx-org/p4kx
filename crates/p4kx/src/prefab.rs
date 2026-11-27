// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};

use anyhow::Result;
use p4kx_assembly::{PrefabError, PrefabIssue, PrefabSet, ResolvedObject, ResolvedPrefab};
use p4kx_extraction::ArchiveError;
use p4kx_extraction::model::PrefabEntityLinkGroup;
use serde::Serialize;

use crate::cli_prefab::{PrefabAction, PrefabArgs, PrefabDumpArgs, PrefabListArgs};
use crate::json;
use crate::query_table::one_line;

#[path = "prefab_dump_origins.rs"]
mod dump_origins;
#[path = "prefab_origins.rs"]
mod origins;
#[path = "prefab_remapping_json.rs"]
mod remapping_json;

#[derive(Serialize)]
struct LibrariesJson<'a> {
    archive: String,
    libraries: Vec<&'a str>,
    #[serde(flatten)]
    origins: origins::Origins,
}

#[derive(Serialize)]
struct PrefabRowJson<'a> {
    name: &'a str,
    id: &'a str,
    objects: usize,
    nested: usize,
    left_out: usize,
}

#[derive(Serialize)]
struct PrefabsJson<'a> {
    archive: String,
    library: &'a str,
    file: &'a str,
    prefabs: Vec<PrefabRowJson<'a>>,
    #[serde(flatten)]
    origins: origins::Origins,
}

#[derive(Serialize)]
struct ObjectJson<'a> {
    name: &'a str,
    kind: &'a str,
    id: &'a str,
    class: &'a str,
    library: &'a str,
    prefab: &'a str,
    container: &'a str,
    depth: usize,
    model: Option<&'a str>,
    unexpanded: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    entity_links: Vec<EntityLinkGroupJson<'a>>,
    attached: bool,
    position: [f32; 3],
    transform: [[f32; 4]; 3],
}

impl<'a> From<&'a ResolvedObject> for ObjectJson<'a> {
    fn from(object: &'a ResolvedObject) -> Self {
        Self {
            name: object.name(),
            kind: object.kind(),
            id: object.id(),
            class: object.class(),
            library: object.library(),
            prefab: object.prefab(),
            container: object.container(),
            depth: object.depth(),
            model: object.model(),
            unexpanded: object.unexpanded().map(|reference| reference.name.as_str()),
            entity_links: object
                .entity_links()
                .iter()
                .map(EntityLinkGroupJson::from)
                .collect(),
            attached: object.attached(),
            position: object.world().translation(),
            transform: object.world().rows,
        }
    }
}

#[derive(Serialize)]
struct EntityLinkGroupJson<'a> {
    links: Vec<EntityLinkJson<'a>>,
}

impl<'a> From<&'a PrefabEntityLinkGroup> for EntityLinkGroupJson<'a> {
    fn from(group: &'a PrefabEntityLinkGroup) -> Self {
        Self {
            links: group
                .links
                .iter()
                .map(|link| EntityLinkJson {
                    name: link.name.as_deref(),
                    target_id: link.target_id.as_deref(),
                    version: link.version.as_deref(),
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct EntityLinkJson<'a> {
    name: Option<&'a str>,
    target_id: Option<&'a str>,
    version: Option<&'a str>,
}

#[derive(Serialize)]
struct IssueJson<'a> {
    container: &'a str,
    name: &'a str,
    kind: &'static str,
    reason: String,
}

impl<'a> From<&'a PrefabIssue> for IssueJson<'a> {
    fn from(issue: &'a PrefabIssue) -> Self {
        Self {
            container: &issue.container,
            name: &issue.name,
            kind: issue.kind.code(),
            reason: issue.kind.to_string(),
        }
    }
}

#[derive(Serialize)]
struct DumpJson<'a> {
    archive: String,
    library: &'a str,
    file: &'a str,
    name: &'a str,
    prefabs_entered: usize,
    objects: Vec<ObjectJson<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    guid_remappings: Vec<remapping_json::InstanceJson<'a>>,
    issues: Vec<IssueJson<'a>>,
    #[serde(flatten)]
    origins: origins::Origins,
}

/// Returns the error as `anyhow` does, with the Archive error of a missing key at the top so that
/// it gets its exit code.
fn lift(error: PrefabError) -> anyhow::Error {
    match error {
        PrefabError::Archive(source) => match source.downcast::<ArchiveError>() {
            Ok(archive) => (*archive).into(),
            Err(other) => anyhow::anyhow!("the Archive cannot be read: {other}"),
        },
        other => other.into(),
    }
}

/// Returns whether the error says the library or the prefab does not exist.
pub(crate) fn is_invalid_input(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<PrefabError>(),
        Some(PrefabError::NoSuchLibrary { .. } | PrefabError::NoSuchPrefab { .. })
    )
}

/// Returns whether the error says a library cannot be read.
pub(crate) fn is_archive_error(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<PrefabError>(),
        Some(PrefabError::Unreadable { .. })
    )
}

/// Runs `p4kx prefab list` or `p4kx prefab dump`.
pub(crate) fn run(args: &PrefabArgs, as_json: bool) -> Result<()> {
    match &args.action {
        PrefabAction::List(list) => list_command(list, as_json),
        PrefabAction::Dump(dump) => dump_command(dump, as_json),
    }
}

fn finish(result: io::Result<()>) -> Result<()> {
    match result {
        Err(error) if error.kind() != io::ErrorKind::BrokenPipe => Err(error.into()),
        _ => Ok(()),
    }
}

fn list_command(args: &PrefabListArgs, as_json: bool) -> Result<()> {
    let archive = crate::key::open(&args.archive)?;
    let mut set = PrefabSet::new(&archive);
    let Some(name) = &args.library else {
        if as_json {
            let (libraries, origins) = origins::index(&archive, &set)?;
            json::print(&LibrariesJson {
                archive: json::path(&args.archive),
                libraries,
                origins,
            })?;
            return Ok(());
        }
        let mut out = BufWriter::new(io::stdout().lock());
        return finish(
            set.libraries()
                .try_for_each(|library| writeln!(out, "{}", one_line(library)))
                .and_then(|()| out.flush()),
        );
    };
    let file = set.library(name).map_err(lift)?;
    let library = file.library();
    let rows: Vec<PrefabRowJson<'_>> = library
        .prefabs
        .iter()
        .map(|prefab| PrefabRowJson {
            name: &prefab.name,
            id: &prefab.id,
            objects: prefab.objects.len(),
            nested: prefab
                .objects
                .iter()
                .filter(|object| object.nested.is_some())
                .count(),
            left_out: prefab.invalid,
        })
        .collect();
    if as_json {
        json::print(&PrefabsJson {
            archive: json::path(&args.archive),
            library: &library.name,
            file: file.file_name(),
            prefabs: rows,
            origins: origins::detail(&archive, &set, &file)?,
        })?;
        return Ok(());
    }
    let mut out = BufWriter::new(io::stdout().lock());
    finish(print_prefabs(&mut out, &rows).and_then(|()| out.flush()))
}

fn print_prefabs(out: &mut impl Write, rows: &[PrefabRowJson<'_>]) -> io::Result<()> {
    writeln!(
        out,
        "{:<38}  {:>8}  {:>6}  {:>8}  Name",
        "Id", "Objects", "Nested", "Left out"
    )?;
    for row in rows {
        writeln!(
            out,
            "{:<38}  {:>8}  {:>6}  {:>8}  {}",
            one_line(row.id),
            row.objects,
            row.nested,
            row.left_out,
            one_line(row.name)
        )?;
    }
    Ok(())
}

fn dump_command(args: &PrefabDumpArgs, as_json: bool) -> Result<()> {
    let archive = crate::key::open(&args.archive)?;
    let mut set = if as_json {
        PrefabSet::with_object_sources(&archive)
    } else {
        PrefabSet::new(&archive)
    };
    let resolved = set.resolve(&args.library, &args.prefab).map_err(lift)?;
    if as_json {
        json::print(&DumpJson {
            archive: json::path(&args.archive),
            library: &resolved.library,
            file: &resolved.file,
            name: &resolved.name,
            prefabs_entered: resolved.prefabs,
            objects: resolved.objects.iter().map(ObjectJson::from).collect(),
            guid_remappings: remapping_json::rows(&resolved),
            issues: resolved.issues.iter().map(IssueJson::from).collect(),
            origins: dump_origins::collect(&archive, &set, &resolved)?,
        })?;
        return Ok(());
    }
    let mut out = BufWriter::new(io::stdout().lock());
    finish(print_dump(&mut out, &resolved).and_then(|()| out.flush()))
}

fn print_dump(out: &mut impl Write, resolved: &ResolvedPrefab) -> io::Result<()> {
    writeln!(
        out,
        "Prefab:   {} / {}",
        one_line(&resolved.library),
        one_line(&resolved.name)
    )?;
    writeln!(out, "Objects:  {}", resolved.objects.len())?;
    writeln!(out, "Entered:  {} prefabs", resolved.prefabs)?;
    writeln!(out, "Issues:   {}", resolved.issues.len())?;
    writeln!(out)?;
    writeln!(
        out,
        "{:<14}  {:<24}  {:>10}  {:>10}  {:>10}  Path",
        "Type", "Class", "X", "Y", "Z"
    )?;
    for object in &resolved.objects {
        let [x, y, z] = object.world().translation();
        let path = if object.container().is_empty() {
            object.name().to_owned()
        } else {
            format!("{}/{}", object.container(), object.name())
        };
        write!(
            out,
            "{:<14}  {:<24}  {x:>10.3}  {y:>10.3}  {z:>10.3}  {}",
            one_line(object.kind()),
            one_line(object.class()),
            one_line(&path)
        )?;
        if let Some(model) = object.model() {
            write!(out, "  {}", one_line(model))?;
        }
        if let Some(reference) = object.unexpanded() {
            write!(out, "  -> {}", one_line(&reference.name))?;
        }
        writeln!(out)?;
    }
    for issue in &resolved.issues {
        let place = if issue.container.is_empty() {
            issue.name.clone()
        } else {
            format!("{}/{}", issue.container, issue.name)
        };
        writeln!(
            out,
            "note {}: {}",
            one_line(&place),
            one_line(&issue.kind.to_string())
        )?;
    }
    Ok(())
}

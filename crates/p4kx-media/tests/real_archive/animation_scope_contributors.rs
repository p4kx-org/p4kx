// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_media::{Identity, MediaRecord};
use serde_json::Value;

use super::sources::Placement;
use super::{TestResult, require};

pub(super) const NAMESPACE: &str = "animation-definition-clip-contributor";

#[path = "animation_scope_contributor_coordinates.rs"]
mod coordinates;

use self::coordinates::{Site, entry_number, number, referenced_entry, string};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Caf,
    Dba,
    Wildcard,
}

impl Kind {
    fn parse(value: &str) -> TestResult<Self> {
        match value {
            "named-caf" => Ok(Self::Caf),
            "named-dba" => Ok(Self::Dba),
            "wildcard" => Ok(Self::Wildcard),
            _ => Err("unexpected alias kind".into()),
        }
    }
}

#[derive(PartialEq, Eq)]
struct Target {
    family: String,
    entry: usize,
    clip: Option<u64>,
}

impl Target {
    fn of(payload: &Value) -> TestResult<Self> {
        Ok(Self {
            family: string(payload, "/kind")?.into(),
            entry: number(payload, "/entry_index")?,
            clip: payload
                .get("clip_ordinal")
                .map(|_| number(payload, "/clip_ordinal"))
                .transpose()?,
        })
    }
}

struct Alias {
    name: Option<String>,
    target: Target,
}

struct Binding {
    owner: u32,
    port: Option<String>,
    root: usize,
}

type Arcs = BTreeMap<usize, BTreeSet<usize>>;

#[derive(Default)]
pub(super) struct Joins {
    clips: BTreeMap<Site, String>,
    names: BTreeMap<Site, String>,
    aliases: BTreeMap<(Site, Kind, usize), Alias>,
    bindings: BTreeMap<usize, Binding>,
    includes: Arcs,
    databases: Arcs,
    spaces: Arcs,
    rows: Vec<Join>,
}

struct Join {
    placement: usize,
    sequence: u64,
    binding: Option<usize>,
    clip: Site,
    alias: (Site, Kind, usize),
    target: Target,
    basis: String,
}

impl Joins {
    pub(super) fn observe(&mut self, row: &MediaRecord) -> TestResult {
        let value = row.data.get("payload").unwrap_or(&Value::Null);
        let kind = row.data.get("record_type").and_then(Value::as_str);
        if row.subresource.namespace == NAMESPACE {
            self.rows.push(Join::read(row, value)?);
        } else if row.subresource.namespace == super::adb::BINDING
            && let [id] = row.subresource.indices.as_slice()
            && let Some(root) = referenced_entry(value)?
        {
            self.bindings.insert(
                usize::try_from(*id)?,
                Binding {
                    owner: u32::try_from(entry_number(value, "/owner_record_index")?)?,
                    port: value
                        .get("port_path")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    root,
                },
            );
        } else if row.subresource.namespace == "animation-definition-sub-database-edge" {
            add(
                &mut self.databases,
                number(value, "/source_entry_index")?,
                number(value, "/target_entry_index")?,
            );
        } else if let Identity::Entry { entry, .. } = &row.source.identity {
            self.document(kind, entry.entry_index, &row.subresource.indices, value)?;
        }
        Ok(())
    }

    fn document(
        &mut self,
        kind: Option<&str>,
        entry: usize,
        indices: &[u64],
        value: &Value,
    ) -> TestResult {
        let [element, declaration, rest @ ..] = indices else {
            return Ok(());
        };
        let site = Site {
            entry,
            element: *element,
            declaration: *declaration,
        };
        match kind {
            Some("clip-reference") => {
                self.clips
                    .insert(site, string(value, "/authored_name")?.into());
            }
            Some("named-caf") => {
                self.names.insert(site, string(value, "/name")?.into());
                if let Some(entry) = referenced_entry(value)? {
                    self.aliases.insert(
                        (site, Kind::Caf, 0),
                        Alias {
                            name: None,
                            target: Target {
                                family: "caf".into(),
                                entry,
                                clip: None,
                            },
                        },
                    );
                }
            }
            Some("named-dba-binding") => self.dba(site, value)?,
            Some("wildcard-candidate") if string(value, "/selection")? == "included" => {
                let [sequence] = rest else {
                    return Err("wildcard candidate identity".into());
                };
                self.aliases.insert(
                    (site, Kind::Wildcard, usize::try_from(*sequence)?),
                    Alias {
                        name: Some(string(value, "/alias")?.into()),
                        target: Target {
                            family: string(value, "/family")?.into(),
                            entry: number(value, "/entry_index")?,
                            clip: None,
                        },
                    },
                );
            }
            Some("link") => {
                if let Some(target) = referenced_entry(value)? {
                    match string(value, "/kind")? {
                        "character-parameters-include" => add(&mut self.includes, entry, target),
                        "combined-space" => add(&mut self.spaces, entry, target),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn dba(&mut self, site: Site, value: &Value) -> TestResult {
        let candidates = value
            .get("candidates")
            .and_then(Value::as_array)
            .ok_or("DBA candidates")?;
        for (ordinal, candidate) in candidates.iter().enumerate() {
            self.aliases.insert(
                (site, Kind::Dba, ordinal),
                Alias {
                    name: None,
                    target: Target {
                        family: "dba".into(),
                        entry: entry_number(candidate, "/entry/entry/entry_index")?,
                        clip: Some(number(candidate, "/clip_ordinal")?),
                    },
                },
            );
        }
        Ok(())
    }

    pub(super) fn verify(&self, placements: &[Placement]) -> TestResult<BTreeMap<usize, usize>> {
        let mut sequences = BTreeMap::new();
        let mut databases = BTreeMap::new();
        let mut scopes = BTreeMap::new();
        let mut blend_matches = BTreeMap::new();
        for join in &self.rows {
            let sequence = sequences.entry(join.placement).or_insert(0);
            require(
                join.sequence == *sequence,
                "contiguous contributor order per placement",
            )?;
            *sequence += 1;
            let placement = placements
                .get(join.placement)
                .ok_or("source-proven placement")?;
            let root = placement
                .parameter
                .ok_or("source-proven placement parameters")?;
            let (parameters, spaces) = scopes.entry(root).or_insert_with(|| self.rig(root));
            require(
                parameters.contains(&join.alias.0.entry),
                "alias belongs to the same parameter scope",
            )?;
            self.verify_binding(join, placement, spaces, &mut databases)?;
            self.verify_match(join)?;
            if join.binding.is_none() {
                *blend_matches.entry(join.clip.entry).or_default() += 1;
            }
        }
        Ok(blend_matches)
    }

    fn rig(&self, root: usize) -> (BTreeSet<usize>, BTreeSet<usize>) {
        let parameters = closure(&self.includes, [root]);
        let roots = self.aliases.iter().filter_map(|((site, _, _), alias)| {
            (parameters.contains(&site.entry)
                && matches!(
                    alias.target.family.as_str(),
                    "blend-space" | "combined-space"
                ))
            .then_some(alias.target.entry)
        });
        let spaces = closure(&self.spaces, roots);
        (parameters, spaces)
    }

    fn verify_binding(
        &self,
        join: &Join,
        placement: &Placement,
        spaces: &BTreeSet<usize>,
        databases: &mut BTreeMap<usize, BTreeSet<usize>>,
    ) -> TestResult {
        let Some(id) = join.binding else {
            return require(
                spaces.contains(&join.clip.entry),
                "clip belongs to a selected blend-space route",
            );
        };
        let binding = self
            .bindings
            .get(&id)
            .ok_or("original physical Entity database binding")?;
        require(
            placement.owner == Some(binding.owner)
                && binding.port.as_deref()
                    == placement
                        .detail
                        .as_ref()
                        .and_then(|detail| detail.get("port_path"))
                        .and_then(Value::as_str),
            "binding belongs to the same physical Entity placement",
        )?;
        let reachable = databases
            .entry(binding.root)
            .or_insert_with(|| closure(&self.databases, [binding.root]));
        require(
            reachable.contains(&join.clip.entry),
            "clip belongs to the original database route",
        )
    }

    fn verify_match(&self, join: &Join) -> TestResult {
        let clip = self
            .clips
            .get(&join.clip)
            .ok_or("original positioned clip declaration")?;
        let alias = self
            .aliases
            .get(&join.alias)
            .ok_or("original positioned alias candidate")?;
        let name = alias
            .name
            .as_ref()
            .or_else(|| self.names.get(&join.alias.0))
            .ok_or("original positioned alias spelling")?;
        require(
            alias.target == join.target,
            "contributor retains the observed physical target",
        )?;
        require(
            !clip.is_empty()
                && clip.eq_ignore_ascii_case(name)
                && join.basis
                    == if clip == name {
                        "exact"
                    } else {
                        "ascii-case-variant"
                    },
            "match basis agrees with verified clip and alias spelling",
        )
    }
}

impl Join {
    fn read(row: &MediaRecord, value: &Value) -> TestResult<Self> {
        let [placement, sequence] = row.subresource.indices.as_slice() else {
            return Err("placement-scoped physical contributor identity".into());
        };
        require(
            matches!(row.source.identity, Identity::Root),
            "directory contributor identity",
        )?;
        require(
            number::<u64>(value, "/placement_ordinal")? == *placement,
            "same placement in identity and payload",
        )?;
        Ok(Self {
            placement: usize::try_from(*placement)?,
            sequence: *sequence,
            binding: value
                .get("database_binding_ordinal")
                .map(|_| number(value, "/database_binding_ordinal"))
                .transpose()?,
            clip: Site::of(value, "/clip")?,
            alias: (
                Site::of(value, "/alias")?,
                Kind::parse(string(value, "/alias/kind")?)?,
                number(value, "/alias/candidate_ordinal")?,
            ),
            target: Target::of(value.get("target").ok_or("original contributor target")?)?,
            basis: string(value, "/match_basis")?.into(),
        })
    }
}

fn add(arcs: &mut Arcs, source: usize, target: usize) {
    arcs.entry(source).or_default().insert(target);
}

fn closure(arcs: &Arcs, roots: impl IntoIterator<Item = usize>) -> BTreeSet<usize> {
    let mut pending = roots.into_iter().collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    while let Some(entry) = pending.pop() {
        if selected.insert(entry)
            && let Some(targets) = arcs.get(&entry)
        {
            pending.extend(targets);
        }
    }
    selected
}

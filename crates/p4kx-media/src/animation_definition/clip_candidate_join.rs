// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use super::super::sub_database_components::Work;
use super::payload;
use super::routes::{Routes, Seed, Walk};
use super::{Alias, ArcKind, Binding, Clip, Observed, Placement};
use crate::input_budget::InputBudget;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::{DirectoryContext, MediaError, MediaWriter, origin_error};

pub(super) fn write(
    observed: &Observed,
    context: &DirectoryContext<'_, '_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let owner = writer
        .origins
        .directory()
        .cloned()
        .ok_or_else(origin_error::foreign)?;
    let budget = context.lookup.budget;
    let index = Index::new(observed, &Work::new(budget, writer))?;
    let output = Output {
        owner: &owner,
        budget,
    };
    for placement in &observed.placements {
        index.write_placement(placement, output, writer)?;
    }
    Ok(())
}

struct Index<'facts> {
    routes: Routes<'facts>,
    bindings: Groups<'facts, u64, Binding>,
    aliases: Groups<'facts, usize, Alias>,
    clips: Groups<'facts, usize, Clip>,
}

impl<'facts> Index<'facts> {
    fn new(observed: &'facts Observed, work: &Work<'_, '_>) -> Result<Self, MediaError> {
        Ok(Self {
            routes: Routes::new(&observed.readable, &observed.arcs, work)?,
            bindings: Groups::new(&observed.bindings, |binding| binding.placement, work)?,
            aliases: Groups::new(&observed.aliases, |alias| alias.site.entry, work)?,
            clips: Groups::new(&observed.clips, |clip| clip.site.entry, work)?,
        })
    }

    fn write_placement(
        &self,
        placement: &'facts Placement,
        output: Output<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let parameters = self.routes.walk(
            ArcKind::Parameters,
            [(placement.parameters, Seed::Root)],
            &output.work(writer),
        )?;
        let aliases = AliasIndex::new(&self.aliases, &parameters, &output.work(writer))?;
        if aliases.names.0.is_empty() {
            return Ok(());
        }
        let mut sequence = 0;
        for &binding in self.bindings.get(&placement.ordinal) {
            let databases = self.routes.walk(
                ArcKind::Database,
                [(binding.root, Seed::Root)],
                &output.work(writer),
            )?;
            let scope = MatchScope {
                placement,
                binding: Some(binding),
                parameters: &parameters,
                definitions: &databases,
            };
            self.write_scope(&scope, &aliases, output, &mut sequence, writer)?;
        }
        let spaces =
            self.routes
                .walk(ArcKind::Space, aliases.space_seeds(), &output.work(writer))?;
        let scope = MatchScope {
            placement,
            binding: None,
            parameters: &parameters,
            definitions: &spaces,
        };
        self.write_scope(&scope, &aliases, output, &mut sequence, writer)
    }

    fn write_scope(
        &self,
        scope: &MatchScope<'_, 'facts>,
        aliases: &AliasIndex<'facts>,
        output: Output<'_>,
        sequence: &mut u64,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        for &entry in scope.definitions.entries() {
            output.work(writer).charge()?;
            for &clip in self.clips.get(&entry) {
                output.work(writer).charge()?;
                for &alias in aliases.names.get(&FoldedName(&clip.authored)) {
                    output.work(writer).charge()?;
                    if scope.binding.is_none() && alias.target.space_entry().is_some() {
                        continue;
                    }
                    let matched = Match {
                        placement: scope.placement,
                        binding: scope.binding,
                        clip,
                        alias,
                    };
                    let proof = scope.proof(&matched, output.budget, writer)?;
                    payload::write(&matched, proof, *sequence, output.owner, writer)?;
                    *sequence = sequence.checked_add(1).ok_or_else(origin_error::counter)?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Output<'scope> {
    owner: &'scope MediaOriginInput,
    budget: &'scope InputBudget,
}

impl Output<'_> {
    const fn work<'run, 'cancel>(
        &'run self,
        writer: &'run MediaWriter<'cancel>,
    ) -> Work<'run, 'cancel> {
        Work::new(self.budget, writer)
    }
}

struct MatchScope<'walk, 'facts> {
    placement: &'facts Placement,
    binding: Option<&'facts Binding>,
    parameters: &'walk Walk<'facts>,
    definitions: &'walk Walk<'facts>,
}

pub(super) struct Match<'facts> {
    pub(super) placement: &'facts Placement,
    pub(super) binding: Option<&'facts Binding>,
    pub(super) clip: &'facts Clip,
    pub(super) alias: &'facts Alias,
}

impl MatchScope<'_, '_> {
    fn proof(
        &self,
        matched: &Match<'_>,
        budget: &InputBudget,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut proof = writer.capture_origins();
        for source in [
            &matched.placement.proof,
            &matched.clip.proof,
            &matched.alias.proof,
            &matched.alias.name_proof,
            &matched.alias.target_proof,
        ] {
            proof.include(source, writer)?;
        }
        if let Some(binding) = matched.binding {
            proof.include(&binding.proof, writer)?;
        }
        self.parameters
            .include(matched.alias.site.entry, &mut proof, budget, writer)?;
        if let Seed::Space(alias) =
            self.definitions
                .include(matched.clip.site.entry, &mut proof, budget, writer)?
        {
            proof.include(&alias.proof, writer)?;
            self.parameters
                .include(alias.site.entry, &mut proof, budget, writer)?;
        }
        Ok(proof.finish())
    }
}

struct AliasIndex<'facts> {
    names: Groups<'facts, FoldedName<'facts>, Alias>,
    spaces: Vec<&'facts Alias>,
}

impl<'facts> AliasIndex<'facts> {
    fn new(
        aliases: &Groups<'facts, usize, Alias>,
        parameters: &Walk<'facts>,
        work: &Work<'_, '_>,
    ) -> Result<Self, MediaError> {
        let mut names = Groups(HashMap::new());
        let mut spaces = Vec::new();
        for &entry in parameters.entries() {
            work.charge()?;
            for &alias in aliases.get(&entry) {
                work.charge()?;
                names.insert(FoldedName(&alias.name), alias, work)?;
                if alias.target.space_entry().is_some() {
                    spaces
                        .try_reserve(1)
                        .map_err(|cause| work.allocation(&cause))?;
                    spaces.push(alias);
                }
            }
        }
        Ok(Self { names, spaces })
    }

    fn space_seeds(&self) -> impl Iterator<Item = (usize, Seed<'facts>)> + '_ {
        self.spaces
            .iter()
            .filter_map(|&alias| Some((alias.target.space_entry()?, Seed::Space(alias))))
    }
}

struct Groups<'facts, Key, Item>(HashMap<Key, Vec<&'facts Item>>);

impl<'facts, Key: Hash + Eq, Item> Groups<'facts, Key, Item> {
    fn new(
        items: &'facts [Item],
        key: impl Fn(&'facts Item) -> Key,
        work: &Work<'_, '_>,
    ) -> Result<Self, MediaError> {
        let mut groups = Self(HashMap::new());
        for item in items {
            work.charge()?;
            groups.insert(key(item), item, work)?;
        }
        Ok(groups)
    }

    fn insert(
        &mut self,
        key: Key,
        item: &'facts Item,
        work: &Work<'_, '_>,
    ) -> Result<(), MediaError> {
        self.0
            .try_reserve(1)
            .map_err(|cause| work.allocation(&cause))?;
        let items = self.0.entry(key).or_default();
        items
            .try_reserve(1)
            .map_err(|cause| work.allocation(&cause))?;
        items.push(item);
        Ok(())
    }

    fn get(&self, key: &Key) -> &[&'facts Item] {
        self.0.get(key).map_or(&[], Vec::as_slice)
    }
}

struct FoldedName<'source>(&'source str);

impl PartialEq for FoldedName<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_ignore_ascii_case(other.0)
    }
}

impl Eq for FoldedName<'_> {}

impl Hash for FoldedName<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(self.0.len());
        for byte in self.0.bytes() {
            state.write_u8(byte.to_ascii_lowercase());
        }
    }
}

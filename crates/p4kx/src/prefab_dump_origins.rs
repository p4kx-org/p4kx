// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "prefab_dump_origins/entity_links.rs"]
mod entity_links;
#[path = "prefab_dump_origins/issue.rs"]
mod issue;
#[path = "prefab_dump_origins/object.rs"]
mod object;
#[path = "prefab_dump_origins/remapping.rs"]
mod remapping;
#[path = "prefab_dump_origins/route.rs"]
mod route;
#[path = "prefab_dump_origins/sites.rs"]
mod sites;

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use p4kx_assembly::{PrefabResolutionSource, PrefabSet, ResolvedPrefab};
use p4kx_extraction::{
    Archive, FieldOrigin, FieldOrigins, SourceUse, SourceUseRef, SourceUseRole, SourceUses,
};

use self::sites::{PrefabAttribute, Proof, Sites};
use super::origins::{self, Origins};

pub(super) fn collect<'archive>(
    archive: &'archive Archive,
    set: &PrefabSet<'archive>,
    resolved: &ResolvedPrefab,
) -> Result<Origins> {
    let source = resolved
        .source()
        .context("prefab dump requires recorded resolution sources")?;
    ensure!(
        source.issues().len() == resolved.issues.len(),
        "prefab issue sources do not match rows"
    );
    let mut sites = Sites::new(archive, set);
    let mut fields = FieldOrigins::new();
    let root = source.root();
    let directory = sites.scope_for_file(root.file)?.clone();
    let file = origins::directory_name(&directory.scope, directory.complete, &sites.uses)?;
    Sites::put_origin(&mut fields, "/file", file)?;
    sites.put(&mut fields, "/archive", &Proof::default())?;
    let mut library = Proof::default();
    library.capture(
        &directory,
        origins::library_name(root.file),
        SourceUseRole::Payload,
    )?;
    sites.put(&mut fields, "/library", &library)?;
    let mut name = Proof::default();
    sites.prefab_attribute(
        &mut name,
        root,
        PrefabAttribute::Name,
        SourceUseRole::Payload,
    )?;
    sites.put(&mut fields, "/name", &name)?;
    let mut diagnostics = collections(&mut sites, &mut fields, source, resolved)?;
    remapping::collect(&mut sites, &mut fields, resolved)?;
    for (index, resolved) in resolved.objects.iter().enumerate() {
        object::collect(&mut sites, &mut fields, index, resolved)?;
    }
    for (index, (row, source)) in resolved.issues.iter().zip(source.issues()).enumerate() {
        diagnostics.add(issue::collect(&mut sites, &mut fields, index, row, source)?);
    }
    sites.put(&mut fields, "/issues", &diagnostics.into_proof())?;
    Ok(Origins {
        field_origins: fields,
        provenance: sites.uses.freeze()?,
    })
}

fn collections(
    sites: &mut Sites<'_, '_>,
    fields: &mut FieldOrigins,
    source: &PrefabResolutionSource,
    resolved: &ResolvedPrefab,
) -> Result<Collection> {
    let root = source.root();
    let mut root_proof = Proof::default();
    sites.objects(&mut root_proof, root, SourceUseRole::Selector)?;
    let mut entered = Collection::default();
    let mut members = Collection::default();
    members.add(root_proof.clone());
    entered.add(root_proof);
    for selection in source.entered() {
        let mut selected = Proof::default();
        route::link(sites, &mut selected, selection.object(), selection.lookup())?;
        entered.add(selected.clone());
        let target = selection
            .lookup()
            .target()
            .context("entered prefab target")?;
        sites.objects(&mut selected, target, SourceUseRole::Selector)?;
        members.add(selected);
    }
    for object in &resolved.objects {
        if let Some(link) = object.unexpanded_source() {
            let mut rejected = Proof::default();
            route::link(sites, &mut rejected, object.location(), link)?;
            entered.add(rejected.clone());
            members.add(rejected);
        }
    }
    sites.put(fields, "/prefabs_entered", &entered.into_proof())?;
    Sites::put_origin(fields, "/objects", members.origin(&sites.uses))?;
    Ok(members)
}

struct Collection {
    uses: BTreeMap<SourceUseRef, SourceUse>,
    complete: bool,
}

impl Default for Collection {
    fn default() -> Self {
        Self {
            uses: BTreeMap::new(),
            complete: true,
        }
    }
}

impl Collection {
    fn add(&mut self, proof: Proof) {
        self.complete &= proof.complete;
        for usage in proof.uses {
            self.uses.entry(usage.reference().clone()).or_insert(usage);
        }
    }

    fn origin(&self, uses: &SourceUses) -> FieldOrigin {
        let sources: Vec<_> = self.uses.values().cloned().collect();
        uses.origin(&sources, self.complete)
    }

    fn into_proof(self) -> Proof {
        Proof {
            uses: self.uses.into_values().collect(),
            complete: self.complete,
        }
    }
}

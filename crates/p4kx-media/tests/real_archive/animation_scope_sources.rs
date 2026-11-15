// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read, Write as _};
use std::sync::Arc;

use p4kx_assembly::{
    ArchiveModels, Attachment, Entities, EntityRig, LoadoutResolver, RigFile, RigFileRead,
    RigInput, RigInputs, RigParameter, RigPlacement, discover_entity_rigs_using,
    discover_model_rig_using,
};
use p4kx_extraction::cryxml::{
    SourceDeclarationKind, SourceDocument, SourceElement, SourceReadOptions,
};
use p4kx_extraction::{Archive, ArchiveWatch, DataForge, DataForgeEntry, Entry};

use serde_json::{Map, Value, json};

use super::adb::{self, Binding};
use super::adb_rows::Graph;
use super::clips::{Clips, Format, Scope};
use super::events::targets as event_targets;
use super::loadout::{Evidence, Files, Observations, unique};
use super::{TestResult, manifests, require};

pub(super) struct Closure<'archive> {
    pub(super) input_entry: usize,
    pub(super) placements: Vec<Placement>,
    pub(super) documents: BTreeMap<usize, Document<'archive>>,
    pub(super) databases: Vec<Binding>,
    pub(super) graph: Graph,
}

pub(super) struct Placement {
    pub(super) owner: Option<u32>,
    pub(super) role: &'static str,
    pub(super) state: &'static str,
    pub(super) parameter: Option<usize>,
    pub(super) detail: Option<Value>,
    pub(super) evidence: Evidence,
}

pub(super) struct Document<'archive> {
    pub(super) entry: &'archive Entry,
    pub(super) bytes: Vec<u8>,
    pub(super) actual: Counts,
    pub(super) clips: Clips,
    expected: Counts,
    family: Family,
}

#[derive(Clone, Copy)]
pub(super) enum Family {
    Params,
    Events,
    Adb,
}

#[derive(Default, PartialEq, Eq)]
pub(super) struct Counts {
    documents: usize,
    declarations: usize,
    timelines: usize,
    events: usize,
    event_attributes: usize,
}

impl<'archive> Closure<'archive> {
    pub(super) fn model(
        archive: &'archive Archive,
        watch: &ArchiveWatch,
        path: &str,
    ) -> TestResult<Self> {
        let entry = unique(archive, path).ok_or("one original model Entry")?;
        let models = ArchiveModels::new(archive);
        let rig =
            discover_model_rig_using(&models, entry.path(), &mut Verified { archive, watch })?;
        let placement = placement(rig.parameters.as_ref(), None, "model")?;
        Self::read(archive, watch, entry.index(), vec![placement], Vec::new())
    }

    pub(super) fn entity(
        archive: &'archive Archive,
        watch: &ArchiveWatch,
        selector: &str,
    ) -> TestResult<Self> {
        let entry = DataForgeEntry::locate(archive).ok_or("original DataForge Entry")?;
        let bytes = verified_bytes(archive, entry, watch)?;
        let forge = DataForge::open(&bytes)?;
        let files = Files { archive, watch };
        let observations = Arc::new(Observations::default());
        let mut resolver = LoadoutResolver::with_observed_entities(
            Arc::new(Entities::new(&forge)),
            observations.clone(),
        )
        .with_files(&files);
        let owner = resolver.entities().find(selector)?;
        let tree = resolver.resolve(&owner)?;
        let mut complete = tree.issues().is_empty();
        tree.visit_ports(|_, route| {
            complete &= match route.last().map(|port| port.attachment()) {
                Some(Attachment::Entity(child)) => child.issues().is_empty(),
                Some(Attachment::Unresolved(_)) => false,
                _ => true,
            };
        });
        require(
            complete,
            "selected real Entity has a fully resolved Archive Loadout",
        )?;
        let models = ArchiveModels::new(archive);
        let rigs = discover_entity_rigs_using(&models, &tree, &mut Verified { archive, watch })?;
        let placements = rigs
            .iter()
            .map(|rig| entity_placement(rig, &observations))
            .collect::<TestResult<Vec<_>>>()?;
        let databases = adb::bindings(archive, &forge, &rigs, entry.index())?;
        Self::read(archive, watch, entry.index(), placements, databases)
    }

    fn read(
        archive: &'archive Archive,
        watch: &ArchiveWatch,
        input_entry: usize,
        placements: Vec<Placement>,
        databases: Vec<Binding>,
    ) -> TestResult<Self> {
        let parameters: BTreeSet<_> = placements
            .iter()
            .filter_map(|item| item.parameter)
            .collect();
        require(
            !parameters.is_empty(),
            "a positive independently resolved parameter scope",
        )?;
        let mut documents = BTreeMap::new();
        let mut event_entries = BTreeSet::new();
        let mut allowance = manifests::MAX_ARTIFACT_BYTES;
        for index in parameters {
            let entry = archive
                .entries()
                .get(index)
                .ok_or("original physical parameter")?;
            let document = Document::read(archive, entry, watch, &mut allowance, Family::Params)?;
            let syntax = SourceDocument::parse(&document.bytes, SourceReadOptions::default())?;
            for name in event_targets(syntax.root()) {
                if let Some(entry) = unique(archive, &name) {
                    event_entries.insert(entry.index());
                }
            }
            documents.insert(index, document);
        }
        for index in event_entries {
            let entry = archive
                .entries()
                .get(index)
                .ok_or("original physical event database")?;
            let document = Document::read(archive, entry, watch, &mut allowance, Family::Events)?;
            documents.insert(index, document);
        }
        let graph = adb::read(archive, watch, &databases, &mut allowance, &mut documents)?;
        Ok(Self {
            input_entry,
            placements,
            documents,
            databases,
            graph,
        })
    }

    pub(super) fn has_events(&self) -> bool {
        self.documents
            .values()
            .any(|document| document.expected.timelines > 0)
    }

    pub(super) fn verify(&self) -> TestResult {
        self.graph.verify()?;
        require(
            self.documents.values().all(|document| {
                document.expected == document.actual && document.clips.remaining() == 0
            }),
            "selected source documents, declarations, timelines, events and clip selectors agree",
        )
    }

    pub(super) fn report(&self) -> TestResult {
        let (mut declarations, mut timelines, mut events) = (0, 0, 0);
        for document in self.documents.values() {
            declarations += document.actual.declarations;
            timelines += document.actual.timelines;
            events += document.actual.events;
        }
        writeln!(
            io::stdout(),
            "verified {} definition Entries, {} placements, {} Entity database bindings, {declarations} declarations, {timelines} timelines, {events} events",
            self.documents.len(),
            self.placements.len(),
            self.databases.len(),
        )?;
        Ok(())
    }
}

fn entity_placement(rig: &EntityRig, observations: &Observations) -> TestResult<Placement> {
    let (role, detail) = match &rig.placement {
        RigPlacement::Root => ("entity-root", None),
        RigPlacement::Attachment { port_path } => (
            "attachment",
            Some(json!({"kind":"attachment", "port_path":port_path})),
        ),
        RigPlacement::LandingGear {
            owner_path,
            ordinal,
            bone,
        } => {
            let mut detail = Map::from_iter([
                ("kind".into(), json!("landing-gear")),
                ("owner_path".into(), json!(owner_path)),
                ("ordinal".into(), json!(ordinal)),
            ]);
            if let Some(bone) = bone {
                detail.insert("bone".into(), json!(bone));
            }
            ("landing-gear", Some(Value::Object(detail)))
        }
        _ => ("other", None),
    };
    let mut placement = placement(rig.parameters.as_ref(), rig.record_index, role)?;
    placement.detail = detail;
    placement.evidence = observations.placement(rig)?;
    Ok(placement)
}

fn placement(
    file: Option<&RigFile<Vec<RigParameter>>>,
    owner: Option<u32>,
    role: &'static str,
) -> TestResult<Placement> {
    let (parameter, state) = match file.map(|file| file.read.as_ref()) {
        None => (None, "no-rig"),
        Some(RigFileRead::Missing) => (None, "no-companion"),
        Some(RigFileRead::Read {
            source: Some(source),
            value: Ok(_),
        }) => (Some(source.entry().index()), "selected"),
        _ => {
            return Err("a known positive placement needs an observable source disposition".into());
        }
    };
    Ok(Placement {
        owner,
        role,
        state,
        parameter,
        detail: None,
        evidence: Evidence::default(),
    })
}

impl<'archive> Document<'archive> {
    pub(super) const fn family(&self) -> &'static str {
        match self.family {
            Family::Params => "chrparams",
            Family::Events => "animevents",
            Family::Adb => "adb",
        }
    }

    pub(super) fn read(
        archive: &'archive Archive,
        entry: &'archive Entry,
        watch: &ArchiveWatch,
        allowance: &mut u64,
        family: Family,
    ) -> TestResult<Self> {
        *allowance = allowance
            .checked_sub(entry.uncompressed_size())
            .ok_or("selected source exceeds the real-Archive artifact budget")?;
        let bytes = verified_bytes(archive, entry, watch)?;
        let source = SourceDocument::parse(&bytes, SourceReadOptions::default())?;
        require(!source.is_lossy(), "complete original source strings")?;
        let expected = Counts::source(source.root(), matches!(family, Family::Events))?;
        let clips = match family {
            Family::Adb => Clips::read(&source, Format::Mannequin, Scope::Entity)?,
            Family::Params | Family::Events => Clips::default(),
        };
        Ok(Self {
            entry,
            bytes,
            actual: Counts::default(),
            clips,
            expected,
            family,
        })
    }
}

impl Counts {
    fn source(root: SourceElement<'_>, events: bool) -> TestResult<Self> {
        let mut counts = Self {
            documents: 1,
            ..Self::default()
        };
        if events && is_exact(root, "anim_event_list") {
            for timeline in root
                .elements()
                .filter(|element| is_exact(*element, "animation"))
            {
                counts.timelines += 1;
                for event in timeline
                    .elements()
                    .filter(|element| is_exact(*element, "event"))
                {
                    counts.events += 1;
                    for declaration in event.syntax()?.declarations() {
                        counts.event_attributes += usize::from(matches!(
                            declaration?.kind(),
                            SourceDeclarationKind::Attribute { .. }
                        ));
                    }
                }
            }
        }
        let mut pending = vec![root];
        while let Some(element) = pending.pop() {
            for declaration in element.syntax()?.declarations() {
                let _declaration = declaration?;
                counts.declarations += 1;
            }
            pending.extend(element.elements());
        }
        Ok(counts)
    }

    pub(super) fn observe(&mut self, namespace: &str) {
        match namespace {
            "animation-definition-document" => self.documents += 1,
            "animation-definition-declaration" => self.declarations += 1,
            "animation-definition-event-timeline" => self.timelines += 1,
            "animation-definition-event" => self.events += 1,
            "animation-definition-event-attribute" => self.event_attributes += 1,
            _ => {}
        }
    }
}

fn is_exact(element: SourceElement<'_>, name: &str) -> bool {
    element.namespace().is_none() && element.name() == name
}

struct Verified<'a> {
    archive: &'a Archive,
    watch: &'a ArchiveWatch,
}

impl RigInputs for Verified<'_> {
    type Error = Box<dyn std::error::Error>;

    fn check(&mut self) -> TestResult {
        self.watch.poll()?;
        Ok(())
    }

    fn read(&mut self, entry: &Entry) -> TestResult<RigInput> {
        verified_bytes(self.archive, entry, self.watch).map(RigInput::Read)
    }
}

fn verified_bytes(archive: &Archive, entry: &Entry, watch: &ArchiveWatch) -> TestResult<Vec<u8>> {
    watch.poll()?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(usize::try_from(entry.uncompressed_size())?)?;
    archive
        .open_entry_verified(entry)?
        .read_to_end(&mut bytes)?;
    require(
        bytes.len() as u64 == entry.uncompressed_size(),
        "verified decoded Entry size",
    )?;
    watch.poll()?;
    Ok(bytes)
}

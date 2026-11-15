// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap};
use std::ops::ControlFlow;
use std::sync::Mutex;

use p4kx_assembly::{
    EntityRig, LoadoutClassSelection, LoadoutFileError, LoadoutFileObservation, LoadoutFileRead,
    LoadoutFiles, LoadoutReadEvent, LoadoutReadObserver, LoadoutSelection, LoadoutXmlDecoding,
    XmlLoadoutClassSite, XmlLoadoutPathSite,
};
use p4kx_extraction::{Archive, ArchiveWatch, Entry};

use super::TestResult;

pub(super) struct Files<'a> {
    pub(super) archive: &'a Archive,
    pub(super) watch: &'a ArchiveWatch,
}

impl LoadoutFiles for Files<'_> {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        self.read_observed(path).into_result()
    }

    fn read_observed(&self, path: &str) -> LoadoutFileRead {
        let Some(entry) = archive_loadout_path(path).and_then(|path| unique(self.archive, &path))
        else {
            return LoadoutFileRead::unbound(path, Err(LoadoutFileError::NotFound));
        };
        LoadoutFileRead::read_archive(self.archive, entry, &|| self.watch.poll().is_err())
    }
}

fn archive_loadout_path(path: &str) -> Option<Cow<'_, str>> {
    let path = path.trim_start_matches(['/', '\\']);
    if path.contains([':', '!', '\0'])
        || path
            .split(['/', '\\'])
            .any(|part| matches!(part, "" | "." | ".."))
        || !path
            .rsplit_once('.')
            .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("xml"))
    {
        return None;
    }
    if path
        .split_once(['/', '\\'])
        .is_some_and(|(root, _)| root.eq_ignore_ascii_case("Data"))
    {
        Some(Cow::Borrowed(path))
    } else {
        Some(Cow::Owned(format!("Data/{path}")))
    }
}

pub(super) fn unique<'archive>(archive: &'archive Archive, path: &str) -> Option<&'archive Entry> {
    let mut entries = archive
        .entries()
        .iter()
        .filter(|entry| folded(entry.path()).eq(folded(path)));
    let entry = entries.next()?;
    (entries.next().is_none() && archive.raw_path(entry.index()).is_none()).then_some(entry)
}

fn folded(path: &str) -> impl Iterator<Item = u8> + '_ {
    path.bytes().map(|byte| {
        if byte == b'\\' {
            b'/'
        } else {
            byte.to_ascii_lowercase()
        }
    })
}

#[derive(Default)]
pub(super) struct Observations(Mutex<Sites>);

#[derive(Default)]
struct Sites {
    paths: HashMap<LoadoutSelection, Site>,
    classes: HashMap<LoadoutClassSelection, Site>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Site {
    pub(super) entry: usize,
    pub(super) namespace: &'static str,
    pub(super) indices: [u64; 2],
    pub(super) field: String,
}

impl Site {
    fn read(
        document: &LoadoutFileObservation,
        decoding: LoadoutXmlDecoding,
        node: u32,
        ordinal: u64,
        attribute: &str,
    ) -> Option<Self> {
        document.read_outcome().ok()?;
        let entry = document.source().identity().entry_index?;
        let namespace = match decoding {
            LoadoutXmlDecoding::Text { .. } => "loadout-xml-attribute",
            LoadoutXmlDecoding::CryXml { .. } => "loadout-cryxml-attribute",
            _ => return None,
        };
        Some(Self {
            entry,
            namespace,
            indices: [u64::from(node), ordinal],
            field: attribute.to_owned(),
        })
    }
}

#[derive(Default)]
pub(super) struct Evidence {
    pub(super) names: BTreeSet<Site>,
    pub(super) selection: BTreeSet<Site>,
}

impl Observations {
    pub(super) fn placement(&self, rig: &EntityRig) -> TestResult<Evidence> {
        let sites = self.0.lock().map_err(|_| "Loadout observation failed")?;
        let names = rig
            .port_selectors()
            .iter()
            .zip(rig.port_name_paths())
            .filter(|(record, _)| record.is_none())
            .filter_map(|(_, path)| path.as_ref())
            .filter_map(|path| sites.paths.get(path.selection()?))
            .cloned()
            .collect();
        let mut selection = BTreeSet::new();
        if let Some(target) = rig.attachment_target()
            && let Some(route) = target.class_xml_use()
        {
            let class = target
                .class_selection()
                .and_then(|selection| sites.classes.get(selection))
                .ok_or("observed original XML class selector")?;
            selection.insert(class.clone());
            for (parent, _, path) in route.includes() {
                let site = path
                    .selection()
                    .and_then(|selection| sites.paths.get(selection))
                    .ok_or("observed original XML Include selector")?;
                if parent.source().identity().entry_index != Some(site.entry) {
                    return Err("Include coordinate belongs to a different original Entry".into());
                }
                selection.insert(site.clone());
            }
        }
        Ok(Evidence { names, selection })
    }
}

impl LoadoutReadObserver for Observations {
    fn observe(&self, _event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn observes_port_names(&self) -> bool {
        true
    }

    fn xml_path(
        &self,
        selection: &LoadoutSelection,
        site: &XmlLoadoutPathSite<'_>,
    ) -> ControlFlow<()> {
        let (Ok(mut sites), Some(source)) = (
            self.0.lock(),
            Site::read(
                site.observation(),
                site.decoding(),
                site.node_index(),
                site.attribute_index(),
                site.attribute_name(),
            ),
        ) else {
            return ControlFlow::Break(());
        };
        sites.paths.insert(selection.clone(), source);
        ControlFlow::Continue(())
    }

    fn xml_class(
        &self,
        selection: &LoadoutClassSelection,
        site: &XmlLoadoutClassSite<'_>,
    ) -> ControlFlow<()> {
        let (Ok(mut sites), Some(source)) = (
            self.0.lock(),
            Site::read(
                site.observation(),
                site.decoding(),
                site.node_index(),
                site.attribute_index(),
                site.attribute_name(),
            ),
        ) else {
            return ControlFlow::Break(());
        };
        sites.classes.insert(selection.clone(), source);
        ControlFlow::Continue(())
    }
}

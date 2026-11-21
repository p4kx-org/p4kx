// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use p4kx_extraction::cryxml::{SourceDocument, SourceElement, SourceReadOptions};
use p4kx_extraction::{Archive, ArchiveWatch, archive_animation_path};

use super::TestResult;
use super::blend_wildcards::{Directory, Pattern, blend_family};
use super::loadout::unique;
use super::sources::{Document, Family};

pub(super) fn read<'archive>(
    archive: &'archive Archive,
    watch: &ArchiveWatch,
    roots: BTreeSet<usize>,
    allowance: &mut u64,
    documents: &mut BTreeMap<usize, Document<'archive>>,
) -> TestResult {
    let mut targets = Targets {
        archive,
        pending: roots
            .into_iter()
            .map(|index| (index, Family::Params))
            .collect(),
        directory: None,
    };
    while let Some((index, family)) = targets.pending.pop_front() {
        let Entry::Vacant(slot) = documents.entry(index) else {
            continue;
        };
        let entry = archive
            .entries()
            .get(index)
            .ok_or("original definition Entry")?;
        let document = Document::read(archive, entry, watch, allowance, family)?;
        let syntax = SourceDocument::parse(&document.bytes, SourceReadOptions::default())?;
        match family {
            Family::Params => targets.character_parameters(syntax.root()),
            Family::CombinedSpace => targets.combined_space(syntax.root()),
            Family::Events | Family::Adb | Family::BlendSpace => {}
        }
        slot.insert(document);
    }
    Ok(())
}

struct Targets<'archive> {
    archive: &'archive Archive,
    pending: VecDeque<(usize, Family)>,
    directory: Option<Directory<'archive>>,
}

impl Targets<'_> {
    fn character_parameters(&mut self, root: SourceElement<'_>) {
        if !is(root, "Params") {
            return;
        }
        for list in root
            .elements()
            .filter(|element| is(*element, "AnimationList"))
        {
            let mut folder = None;
            for element in list.elements().filter(|element| is(*element, "Animation")) {
                let name = unique_attribute(element, "name");
                let path = unique_attribute(element, "path");
                if folder_directive(element) {
                    folder = name.and(path);
                } else if let (Some(name), Some(path)) = (name, path) {
                    self.animation(name, path, folder);
                }
            }
        }
    }

    fn animation(&mut self, name: &str, path: &str, folder: Option<&str>) {
        let extension = path.rsplit_once('.').map(|(_, extension)| extension);
        if name.eq_ignore_ascii_case("$AnimEventDatabase")
            && extension.is_some_and(|value| value.eq_ignore_ascii_case("animevents"))
        {
            self.exact(path, Family::Events);
        } else if name.eq_ignore_ascii_case("$Include")
            && extension.is_some_and(|value| value.eq_ignore_ascii_case("chrparams"))
        {
            self.exact(path, Family::Params);
        } else if let Some(pattern) = Pattern::parse(name, path, folder) {
            let directory = self
                .directory
                .get_or_insert_with(|| Directory::read(self.archive));
            self.pending.extend(directory.matches(&pattern));
        }
    }

    fn combined_space(&mut self, root: SourceElement<'_>) {
        if !exact_element(root, "CombinedBlendSpace") {
            return;
        }
        for list in root
            .elements()
            .filter(|element| exact_element(*element, "BlendSpaces"))
        {
            for entry in list
                .elements()
                .filter(|element| exact_element(*element, "BlendSpace"))
            {
                let Some(path) = unique_attribute(entry, "AName") else {
                    continue;
                };
                if let Some(family) = path
                    .rsplit_once('.')
                    .and_then(|(_, extension)| blend_family(extension))
                {
                    self.exact(path, family);
                }
            }
        }
    }

    fn exact(&mut self, authored: &str, family: Family) {
        if let Some(path) = archive_animation_path(authored)
            && let Some(entry) = unique(self.archive, &path)
        {
            self.pending.push_back((entry.index(), family));
        }
    }
}

fn folder_directive(element: SourceElement<'_>) -> bool {
    element.attributes().any(|attribute| {
        attribute.namespace().is_none()
            && attribute.name().eq_ignore_ascii_case("name")
            && attribute.value().eq_ignore_ascii_case("#filepath")
    })
}

fn unique_attribute<'source>(
    element: SourceElement<'source>,
    wanted: &str,
) -> Option<&'source str> {
    let mut attributes = element.attributes().filter(|attribute| {
        attribute.namespace().is_none() && attribute.name().eq_ignore_ascii_case(wanted)
    });
    let attribute = attributes.next()?;
    attributes.next().is_none().then(|| attribute.value())
}

fn is(element: SourceElement<'_>, name: &str) -> bool {
    element.namespace().is_none() && element.name().eq_ignore_ascii_case(name)
}

fn exact_element(element: SourceElement<'_>, name: &str) -> bool {
    element.namespace().is_none() && element.name() == name
}

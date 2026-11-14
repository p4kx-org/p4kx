// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

use p4kx_extraction::chunks::ChunkFile;
use p4kx_extraction::cryxml::{SourceDocument, SourceElement, SourceReadOptions};
use p4kx_extraction::{Archive, Entry, FieldOrigin, SourceEncryption};
use p4kx_model::{CafImage, CafImagePath, DbaMetadataRecords};

use super::{BodyVariant, CancelToken, PathIndex, SourceUses, XmlDocuments, read};
use crate::player_rig_clip::ClipSources;
use crate::{PlayerDocument, PlayerDocumentSink};

type Check<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct Sink;

impl PlayerDocumentSink for Sink {
    fn document(&mut self, _document: PlayerDocument<'_>) -> std::io::Result<()> {
        Ok(())
    }
}

struct OriginalRule {
    folder: String,
    banks: BTreeMap<u32, String>,
}

#[test]
#[ignore = "installed Player rig and CAF/DBA witness; set P4KX_DATA_P4K"]
fn installed_rig_clip_paths_and_ordinals_match_original_sources() -> Check {
    let archive = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )
    .map_err(|_| "cannot open installed Archive")?;
    let cancel = CancelToken::new();
    let sources = SourceUses::with_compact_use_refs();
    let directory = sources
        .observe_root_directory_paths(&archive)?
        .ok_or("root directory")?;
    let index = PathIndex::new(&archive);
    let results = read(
        &index,
        &BodyVariant::ALL,
        &sources,
        &directory,
        &cancel,
        &mut XmlDocuments::stream_to(&mut Sink),
    )
    .map_err(|_| "installed clip candidate capture failed")?;
    let image = index
        .find("Data/Animations/Animations.img")
        .ok_or("original image missing")?;
    let image_bytes = member(&archive, image)?;
    let image_file = ChunkFile::parse(image_bytes.as_slice())?;
    let rules = BodyVariant::ALL.map(|variant| read_rules(&index, &archive, variant));
    let (Some(male), Some(female)) = (rules.first(), rules.last()) else {
        return Err("original ordered body selectors unavailable".into());
    };
    let expected = [
        male.as_ref().map_err(|_| "first authored rig failed")?,
        female.as_ref().map_err(|_| "second authored rig failed")?,
    ];
    let context = Originals {
        image: &image_file,
        image_encryption: encrypted(&archive, image)?,
        rules: expected,
        index: &index,
        archive: &archive,
    };
    context.verify(&results)
}

struct Originals<'a, 'bytes> {
    image: &'a ChunkFile<&'bytes [u8]>,
    image_encryption: bool,
    rules: [&'a BTreeMap<(usize, usize), OriginalRule>; 2],
    index: &'a PathIndex<'a>,
    archive: &'a Archive,
}

impl Originals<'_, '_> {
    fn verify(&self, results: &super::RigClips) -> Check {
        let mut bank_records: BTreeMap<String, (bool, Vec<String>)> = BTreeMap::new();
        let rules: BTreeMap<_, _> = results
            .rules
            .iter()
            .map(|(rig, index, rule)| ((*rig, *index), (rule.list, rule.child)))
            .collect();
        let image_records = self.image_records()?;
        let mut proved = BTreeSet::new();
        for (rig, clip) in &results.clips {
            let coordinate = rules
                .get(&(*rig, clip.rule))
                .ok_or("unaddressed original rule")?;
            let rule = self
                .rules
                .get(*rig)
                .and_then(|rules| rules.get(coordinate))
                .ok_or("missing scoped original rule")?;
            let record = image_records
                .get(&clip.chunk)
                .and_then(|rows| rows.get(clip.record))
                .ok_or("missing original physical image record")?;
            let CafImagePath::Text(path) = record.path() else {
                return Err("unreadable original indexed path".into());
            };
            let crc = record
                .dba_path_crc32()
                .ok_or("original DBA CRC absent")?
                .value;
            let canonical = normalize(path);
            if clip.path.value().map(String::as_str) != Some(path)
                || clip.bank_crc32.value() != Some(&crc)
                || !canonical
                    .strip_prefix(&rule.folder)
                    .is_some_and(|tail| tail.starts_with('/'))
            {
                return Err("original path, CRC or scoped folder mismatch".into());
            }
            verify_encryption(clip.path.origin(), self.image_encryption)?;
            if let Some(ClipSources::Database { candidates }) = clip.source.value()
                && let Some(bank) = rule.banks.get(&crc)
            {
                let (encrypted, names) = self.bank(&mut bank_records, bank)?;
                let names = names
                    .iter()
                    .enumerate()
                    .filter(|(_, name)| normalize(name) == canonical)
                    .map(|(ordinal, _)| ordinal);
                let actual = candidates.iter().flat_map(|candidate| &candidate.records);
                if actual.clone().map(|record| record.ordinal).ne(names) {
                    return Err("original full-path DBA ordinal mismatch".into());
                }
                for record in actual {
                    verify_encryption(record.name.origin(), *encrypted)?;
                    proved.insert(*rig);
                }
            }
        }
        (proved.len() == BodyVariant::ALL.len())
            .then_some(())
            .ok_or_else(|| "missing exact physical DBA coverage for a selected rig".into())
    }

    fn image_records(&self) -> Check<BTreeMap<usize, Vec<p4kx_model::CafImageRecord<'_>>>> {
        self.image
            .chunks()
            .iter()
            .enumerate()
            .filter(|(_, chunk)| chunk.chunk_type().id() == 0x3702_75B7 && chunk.version() == 0x902)
            .map(|(ordinal, chunk)| {
                let bytes = self
                    .image
                    .chunk_data(ordinal)
                    .ok_or("missing CAF image bytes")?;
                let image = CafImage::read(bytes, chunk.offset())?;
                Ok::<_, Box<dyn std::error::Error>>((ordinal, image.records().collect::<Vec<_>>()))
            })
            .collect()
    }

    fn bank<'cache>(
        &self,
        cache: &'cache mut BTreeMap<String, (bool, Vec<String>)>,
        path: &str,
    ) -> Check<&'cache (bool, Vec<String>)> {
        use std::collections::btree_map::Entry;
        Ok(match cache.entry(path.to_owned()) {
            Entry::Occupied(existing) => existing.into_mut(),
            Entry::Vacant(slot) => {
                let entry = self.index.find(path).ok_or("declared bank unavailable")?;
                let bytes = member(self.archive, entry)?;
                let file = ChunkFile::parse(bytes)?;
                let names = DbaMetadataRecords::read(path, &file)
                    .map_err(|_| "original bank metadata framing unavailable")?
                    .map(|record| {
                        let record = record.map_err(|_| "original bank metadata malformed")?;
                        std::str::from_utf8(record.name.bytes())
                            .map(str::to_owned)
                            .map_err(|_| "original bank name unavailable")
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                slot.insert((encrypted(self.archive, entry)?, names))
            }
        })
    }
}

fn read_rules(
    index: &PathIndex<'_>,
    archive: &Archive,
    body: BodyVariant,
) -> Check<BTreeMap<(usize, usize), OriginalRule>> {
    let stem = body
        .skeleton()
        .strip_suffix(".chr")
        .ok_or("missing skeleton selector")?;
    let params = index
        .find(&format!("Data/{stem}.chrparams"))
        .ok_or("adjacent parameters missing")?;
    let bytes = member(archive, params)?;
    let document = SourceDocument::parse(&bytes, SourceReadOptions::default())
        .map_err(|_| "original parameters malformed")?;
    let mut rules = BTreeMap::new();
    for (ordinal, list) in document
        .root()
        .elements()
        .enumerate()
        .filter(|(_, list)| list.name() == "AnimationList")
    {
        let mut banks = BTreeMap::new();
        for item in list.elements().filter(|item| item.name() == "Animation") {
            if attribute(item, "name")
                .is_some_and(|name| name.eq_ignore_ascii_case("$TracksDatabase"))
                && let Some(path) = attribute(item, "path")
            {
                let path = normalize(path);
                let local = path.strip_prefix("data/").ok_or("bank path outside Data")?;
                banks.insert(crc32fast::hash(local.as_bytes()), path);
            }
        }
        let mut folder = None;
        for (child, item) in list
            .elements()
            .enumerate()
            .filter(|(_, item)| item.name() == "Animation")
        {
            let (Some(name), Some(path)) = (attribute(item, "name"), attribute(item, "path"))
            else {
                continue;
            };
            if name.eq_ignore_ascii_case("#filepath") {
                folder = Some(normalize(path).trim_end_matches('/').to_owned());
            } else if name == "*"
                && path.replace('\\', "/").eq_ignore_ascii_case("*/*.caf")
                && let Some(folder) = &folder
            {
                rules.insert(
                    (ordinal, child),
                    OriginalRule {
                        folder: folder.clone(),
                        banks: banks.clone(),
                    },
                );
            }
        }
    }
    Ok(rules)
}

fn attribute<'a>(item: SourceElement<'a>, name: &str) -> Option<&'a str> {
    let mut exact = item
        .attributes()
        .filter(|attr| attr.name() == name && attr.namespace().is_none());
    let value = exact.next()?.value();
    exact.next().is_none().then_some(value)
}

fn normalize(path: &str) -> String {
    let path = path.replace('\\', "/").to_ascii_lowercase();
    if path.starts_with("data/") {
        path
    } else {
        format!("data/{path}")
    }
}

fn encrypted(archive: &Archive, entry: &Entry) -> Check<bool> {
    let origin = archive
        .entry_origin(entry)
        .ok_or("missing native archive position")?;
    match origin.encryption() {
        SourceEncryption::Encrypted => Ok(true),
        SourceEncryption::Unencrypted => Ok(false),
        _ => Err("unknown native archive encryption".into()),
    }
}

fn verify_encryption(origin: &FieldOrigin, expected: bool) -> Check {
    if origin.contains_encrypted_source() != Some(expected) || !origin.source_set_complete() {
        return Err("native physical encryption differs from emitted field origin".into());
    }
    Ok(())
}

fn member(archive: &Archive, entry: &Entry) -> Check<Vec<u8>> {
    let mut reader = archive
        .open_entry_verified(entry)
        .map_err(|_| "cannot open original verified input")?;
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read original verified input")?;
    Ok(bytes)
}

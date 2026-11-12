// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::ByteReader;

use crate::hierarchy::find_cycle;
use crate::reader::{ReadValues, to_usize};
use crate::{Bone, BoneTransform, CorruptChunk, ModelBoneSource, ModelBoneSources};

pub(crate) const ENTRY_LEN: usize = 68;
pub(crate) const COMPACT_HEADER_LEN: usize = 48;
const COMPACT_HEADER_USED: usize = 8;
pub(crate) const COMPACT_RECORD_LEN: usize = 16;
const POSE_LEN: usize = 28;
const NODE_AT: usize = 14;
const PARENT_AT: usize = 8;

/// Reads a `CompiledBones2` chunk: a bone count, one 68-byte entry per bone (controller ID,
/// limb ID, parent, relative and world pose) and the NUL-terminated bone names.
pub(crate) fn read_bones(
    data: &[u8],
    sources: Option<&mut ModelBoneSources>,
) -> Result<Vec<Bone>, CorruptChunk> {
    let mut reader = ByteReader::new(data);
    let count = to_usize(reader.read_u32()?);
    let record_start = reader.offset();
    let entries = reader.read_bytes(count.saturating_mul(ENTRY_LEN))?;
    let names_start = reader.offset();
    let relative_start = record_start + PARENT_AT as u64 + size_of::<u32>() as u64;
    let capture = sources.map(|sources| NameCapture {
        sources,
        fields: BoneFields {
            names_start,
            records_start: record_start,
            record_stride: ENTRY_LEN,
            parent_width: size_of::<u32>() as u64,
            node: false,
            relative_start,
            world_start: relative_start + POSE_LEN as u64,
            pose_stride: ENTRY_LEN,
        },
    });
    let names = names(reader.read_bytes(reader.remaining())?, count, capture)?;
    let mut bones = Vec::with_capacity(count);
    for (entry, name) in entries.as_chunks::<ENTRY_LEN>().0.iter().zip(names) {
        let mut fields = ByteReader::new(entry);
        let controller_id = fields.read_u32()?;
        fields.read_u32()?;
        let parent = i32::from_le_bytes(fields.read_u32()?.to_le_bytes());
        let parent = parent_index(parent, count, bones.len())?;
        let relative = read_pose(&mut fields)?;
        let world = read_pose(&mut fields)?;
        bones.push(Bone {
            name,
            parent,
            controller_id,
            node: None,
            relative,
            world,
        });
    }
    check_acyclic(&bones)?;
    Ok(bones)
}

/// Reads a `CompiledBones` chunk: a header with the bone count and the size of the name table,
/// one 16-byte record per bone (controller ID, limb ID, child count, parent, node index), the
/// names, then the relative and the world poses of all bones.
pub(crate) fn read_compact_bones(
    data: &[u8],
    sources: Option<&mut ModelBoneSources>,
) -> Result<Vec<Bone>, CorruptChunk> {
    let mut reader = ByteReader::new(data);
    let count = to_usize(reader.read_u32()?);
    let names_len = to_usize(reader.read_u32()?);
    reader.skip(COMPACT_HEADER_LEN - COMPACT_HEADER_USED)?;
    let record_start = reader.offset();
    let records = reader.read_bytes(count.saturating_mul(COMPACT_RECORD_LEN))?;
    let names_start = reader.offset();
    let name_bytes = reader.read_bytes(names_len)?;
    let relative_start = reader.offset();
    let capture = sources.map(|sources| NameCapture {
        sources,
        fields: BoneFields {
            names_start,
            records_start: record_start,
            record_stride: COMPACT_RECORD_LEN,
            parent_width: size_of::<u16>() as u64,
            node: true,
            relative_start,
            world_start: relative_start + count as u64 * POSE_LEN as u64,
            pose_stride: POSE_LEN,
        },
    });
    let names = names(name_bytes, count, capture)?;
    let relative = reader.read_bytes(count.saturating_mul(POSE_LEN))?;
    let world = reader.read_bytes(count.saturating_mul(POSE_LEN))?;
    let mut bones = Vec::with_capacity(count);
    let poses = relative
        .as_chunks::<POSE_LEN>()
        .0
        .iter()
        .zip(world.as_chunks::<POSE_LEN>().0);
    for ((record, name), (relative, world)) in records
        .as_chunks::<COMPACT_RECORD_LEN>()
        .0
        .iter()
        .zip(names)
        .zip(poses)
    {
        let mut fields = ByteReader::new(record);
        let controller_id = fields.read_u32()?;
        let parent = i16_at(record, PARENT_AT);
        let node = i16_at(record, NODE_AT);
        bones.push(Bone {
            name,
            parent: parent_index(i32::from(parent), count, bones.len())?,
            controller_id,
            node: usize::try_from(node).ok(),
            relative: read_pose(&mut ByteReader::new(relative))?,
            world: read_pose(&mut ByteReader::new(world))?,
        });
    }
    check_acyclic(&bones)?;
    Ok(bones)
}

fn i16_at(bytes: &[u8], at: usize) -> i16 {
    let pair = bytes
        .get(at..at + 2)
        .and_then(|pair| <[u8; 2]>::try_from(pair).ok());
    pair.map_or(0, i16::from_le_bytes)
}

fn read_pose(reader: &mut ByteReader<'_>) -> Result<BoneTransform, CorruptChunk> {
    let rotation = [
        reader.read_f32()?,
        reader.read_f32()?,
        reader.read_f32()?,
        reader.read_f32()?,
    ];
    let translation = reader.read_vec3()?;
    if rotation
        .iter()
        .chain(&translation)
        .any(|value| !value.is_finite())
    {
        return Err(CorruptChunk::NonFinite("bone transform"));
    }
    Ok(BoneTransform {
        rotation,
        translation,
    })
}

fn parent_index(parent: i32, bones: usize, bone: usize) -> Result<Option<usize>, CorruptChunk> {
    if parent < 0 {
        return Ok(None);
    }
    match usize::try_from(parent) {
        Ok(parent) if parent < bones => Ok(Some(parent)),
        _ => Err(CorruptChunk::BoneParent {
            bone,
            parent,
            bones,
        }),
    }
}

struct BoneFields {
    names_start: u64,
    records_start: u64,
    record_stride: usize,
    parent_width: u64,
    node: bool,
    relative_start: u64,
    world_start: u64,
    pose_stride: usize,
}

impl BoneFields {
    fn observe(&mut self, sources: &mut ModelBoneSources, ordinal: usize, name_extent: usize) {
        let name_start = sources.start + self.names_start;
        self.names_start += name_extent as u64;
        let record = sources.start + self.records_start + (ordinal * self.record_stride) as u64;
        let parent = record + PARENT_AT as u64;
        let node = record + NODE_AT as u64;
        let pose = |start| {
            let offset = sources.start + start + (ordinal * self.pose_stride) as u64;
            offset..offset + POSE_LEN as u64
        };
        sources.fields.push(ModelBoneSource {
            name: name_start..sources.start + self.names_start,
            parent: parent..parent + self.parent_width,
            controller: record..record + size_of::<u32>() as u64,
            node: self.node.then_some(node..node + size_of::<u16>() as u64),
            relative: pose(self.relative_start),
            world: pose(self.world_start),
        });
    }
}

struct NameCapture<'a> {
    sources: &'a mut ModelBoneSources,
    fields: BoneFields,
}

fn names(
    table: &[u8],
    count: usize,
    mut capture: Option<NameCapture<'_>>,
) -> Result<Vec<String>, CorruptChunk> {
    let mut parts = table.split_inclusive(|&byte| byte == 0);
    (0..count)
        .map(|ordinal| {
            let (_, name) = parts
                .next()
                .and_then(<[u8]>::split_last)
                .filter(|&(&last, _)| last == 0)
                .ok_or(CorruptChunk::BoneNames { bones: count })?;
            if let Some(capture) = capture.as_mut() {
                capture
                    .fields
                    .observe(capture.sources, ordinal, name.len() + 1);
            }
            Ok(String::from_utf8_lossy(name).into_owned())
        })
        .collect()
}

fn check_acyclic(bones: &[Bone]) -> Result<(), CorruptChunk> {
    let parents: Vec<Option<usize>> = bones.iter().map(|bone| bone.parent).collect();
    find_cycle(&parents).map_or(Ok(()), |bone| Err(CorruptChunk::BoneCycle(bone)))
}

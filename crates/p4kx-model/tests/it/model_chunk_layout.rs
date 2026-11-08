// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_chunks::{ByteOrder, ChunkFile, crch, ivo};
use p4kx_model::{
    ChunkUsage, CompanionError, CompanionPath, LodSelection, LodSkeletonOrigin, MaterialReading,
    Model, ModelFile, PositionBox,
};
use p4kx_testkit::{TestChunkFile, TestModel, TestSkeleton};

type TestResult = Result<(), Box<dyn Error>>;
const UNKNOWN_IVO: u32 = 0xDEAD_BEEF;
const UNKNOWN_CRCH: u32 = 0xABCD;
const HEADER_BYTES: usize = 16;
const TABLE_ENTRY_BYTES: usize = 16;
const IVO_OFFSET_POSITION: usize = 8;

fn read(file: &ChunkFile<Vec<u8>>) -> Result<Model, Box<dyn Error>> {
    let mut companions =
        |_: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> { Ok(None) };
    Ok(Model::read_with_materials(
        "source.cgf",
        file,
        &mut companions,
        LodSelection::All,
        PositionBox::default(),
        MaterialReading::Skipped,
    )?)
}

fn assert_directory(model: &Model, file: &ChunkFile<Vec<u8>>, role: ModelFile) {
    let actual: Vec<_> = model
        .chunks
        .iter()
        .filter(|chunk| chunk.file == role)
        .collect();
    assert_eq!(
        actual.len(),
        file.chunks().len(),
        "every actual table ordinal"
    );
    for ((index, expected), actual) in file.chunks().iter().enumerate().zip(actual) {
        assert_eq!(
            (actual.index, actual.chunk_type),
            (index, expected.chunk_type()),
            "source ordinal and type"
        );
        assert_eq!(actual.layout.version, expected.version(), "parsed version");
        assert_eq!(actual.layout.id, expected.id(), "declared ID or absent ID");
        assert_eq!(
            actual.layout.byte_order,
            expected.byte_order(),
            "data byte order"
        );
        assert_eq!(
            actual.layout.offset,
            expected.offset(),
            "declared physical offset"
        );
        assert_eq!(
            actual.layout.size,
            expected.size(),
            "same owning parser's byte span"
        );
    }
}

#[test]
fn ivo_skipped_and_unknown_chunks_keep_version_order_and_derived_padding() -> TestResult {
    let bytes = TestChunkFile::ivo()
        .chunk(ivo::LOD_DISTANCES.id(), 0x1234_5678, b"abc".to_vec())
        .chunk(UNKNOWN_IVO, u32::MAX, b"opaque".to_vec())
        .table_at_end()
        .bytes();
    let file = ChunkFile::parse(bytes)?;
    let model = read(&file)?;
    assert_directory(&model, &file, ModelFile::Model);
    let [known, unknown] = model.chunks.as_slice() else {
        return Err("two physical chunk observations".into());
    };
    assert_eq!(
        known.usage,
        ChunkUsage::Skipped,
        "known opaque payload is not decoded"
    );
    assert_eq!(
        known.layout.version, 0x1234_5678,
        "uninterpreted version survives"
    );
    assert_eq!(known.layout.size, 8, "Ivo span includes alignment padding");
    assert_eq!(known.layout.id, None, "Ivo has no declared ID");
    assert_eq!(
        unknown.usage,
        ChunkUsage::Unknown,
        "unknown type remains explicit"
    );
    assert_eq!(
        unknown.layout.version,
        u32::MAX,
        "unknown version is not truncated"
    );
    Ok(())
}

#[test]
fn ivo_alias_offsets_keep_both_original_ordinals_and_the_same_inferred_span() -> TestResult {
    let fixture = TestChunkFile::ivo()
        .chunk(ivo::LOD_DISTANCES.id(), 7, b"abc".to_vec())
        .chunk(UNKNOWN_IVO, 11, b"opaque".to_vec());
    let first = fixture.offsets().first().copied().ok_or("first offset")?;
    let mut bytes = fixture.bytes();
    let position = HEADER_BYTES + TABLE_ENTRY_BYTES + IVO_OFFSET_POSITION;
    bytes
        .get_mut(position..position + 8)
        .ok_or("second table offset")?
        .copy_from_slice(&first.to_le_bytes());
    let file = ChunkFile::parse(bytes)?;
    let model = read(&file)?;
    assert_directory(&model, &file, ModelFile::Model);
    let [first, second] = model.chunks.as_slice() else {
        return Err("both aliased table entries".into());
    };
    assert_eq!(
        (first.index, second.index),
        (0, 1),
        "no physical-ordinal deduplication"
    );
    assert_eq!(
        first.layout.offset, second.layout.offset,
        "declared alias retained"
    );
    assert_eq!(
        first.layout.size, second.layout.size,
        "one inferred boundary span"
    );
    assert_eq!(
        file.chunk_data(0),
        file.chunk_data(1),
        "raw slices remain identical"
    );
    Ok(())
}

#[test]
fn crch_keeps_declared_size_id_and_separate_endian_flag_interpretation() -> TestResult {
    let file = ChunkFile::parse(
        TestChunkFile::crch()
            .chunk(crch::MESH_PHYSICS_DATA.id(), 0x0800, b"unused".to_vec())
            .big_endian_chunk(UNKNOWN_CRCH, 0x1234, b"abc".to_vec())
            .bytes(),
    )?;
    let model = read(&file)?;
    assert_directory(&model, &file, ModelFile::Model);
    let second = model.chunks.get(1).ok_or("big-endian opaque chunk")?;
    assert_eq!(
        second.layout.version, 0x1234,
        "version excludes the table endian flag"
    );
    assert_eq!(
        second.layout.byte_order,
        ByteOrder::Big,
        "flag meaning remains separate"
    );
    assert_eq!(second.layout.id, Some(2), "actual table ID");
    assert_eq!(second.layout.size, 3, "CrCh declares the exact byte length");
    assert_eq!(
        second.usage,
        ChunkUsage::Unknown,
        "opaque endian data is not decoded"
    );
    Ok(())
}

fn level_model() -> TestModel {
    TestModel::cube()
        .with_skeleton(&TestSkeleton::chain(6))
        .with_bone_map(false, vec![vec![(5, 255)]; 24])
        .with_chunk(UNKNOWN_IVO, b"level opaque".to_vec())
}

struct Directory {
    role: ModelFile,
    file: ChunkFile<Vec<u8>>,
}

fn directories(fixture: &TestModel) -> Result<[Directory; 4], Box<dyn Error>> {
    Ok([
        Directory {
            role: ModelFile::Model,
            file: ChunkFile::parse(fixture.cgf())?,
        },
        Directory {
            role: ModelFile::Geometry,
            file: ChunkFile::parse(fixture.cgfm())?,
        },
        Directory {
            role: ModelFile::LodGeometry(1),
            file: ChunkFile::parse(fixture.lod_cgfm(1).ok_or("level geometry")?)?,
        },
        Directory {
            role: ModelFile::LodModel(1),
            file: ChunkFile::parse(fixture.lod_cgf(1).ok_or("level model")?)?,
        },
    ])
}

#[test]
fn every_actually_read_root_geometry_and_level_model_keeps_its_own_layout() -> TestResult {
    let fixture = TestModel::two_bone_skin().with_lod(level_model());
    let files = directories(&fixture)?;
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Sibling("skin.skinm") => Some(fixture.cgfm()),
            CompanionPath::Sibling("skin_lod1.skinm") => fixture.lod_cgfm(1),
            CompanionPath::Sibling("skin_lod1.skin") => fixture.lod_cgf(1),
            _ => None,
        })
    };
    let root = &files.first().ok_or("root file")?.file;
    let model = Model::read_with_materials(
        "skin.skin",
        root,
        &mut companions,
        LodSelection::All,
        PositionBox::default(),
        MaterialReading::Skipped,
    )?;
    for directory in &files {
        assert_directory(&model, &directory.file, directory.role);
    }
    let level = model.lods.get(1).ok_or("level one")?;
    assert_eq!(
        model.bones_of(level).len(),
        6,
        "effective level skeleton is unchanged"
    );
    assert_eq!(
        level.skeleton_origin,
        LodSkeletonOrigin::Distinct,
        "actual lookup state"
    );
    assert!(
        model
            .chunks
            .iter()
            .any(|chunk| chunk.file == ModelFile::LodModel(1)
                && chunk.chunk_type.id() == UNKNOWN_IVO),
        "level model's unknown chunk survives"
    );
    Ok(())
}

#[test]
fn an_unread_level_model_does_not_gain_an_invented_physical_directory() -> TestResult {
    let fixture = TestModel::two_bone_skin().with_lod(TestModel::cube());
    let root = ChunkFile::parse(fixture.cgf())?;
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Sibling("skin.skinm") => Some(fixture.cgfm()),
            CompanionPath::Sibling("skin_lod1.skinm") => fixture.lod_cgfm(1),
            CompanionPath::Sibling("skin_lod1.skin") => fixture.lod_cgf(1),
            _ => None,
        })
    };
    let model = Model::read_with_materials(
        "skin.skin",
        &root,
        &mut companions,
        LodSelection::All,
        PositionBox::default(),
        MaterialReading::Skipped,
    )?;
    assert_eq!(
        model.lods.get(1).ok_or("level one")?.skeleton_origin,
        LodSkeletonOrigin::NotRead,
        "not requested is distinct from missing"
    );
    assert!(
        model
            .chunks
            .iter()
            .all(|chunk| chunk.file != ModelFile::LodModel(1)),
        "unopened files have no fabricated table fields"
    );
    Ok(())
}

#[test]
fn a_visareas_only_level_keeps_layout_even_without_an_output_lod() -> TestResult {
    let fixture = TestModel::two_bone_skin();
    let opaque = TestChunkFile::ivo()
        .chunk(ivo::VIS_AREAS.id(), 17, b"areas".to_vec())
        .chunk(UNKNOWN_IVO, 19, b"uninterpreted".to_vec())
        .bytes();
    let expected = ChunkFile::parse(opaque.clone())?;
    let root = ChunkFile::parse(fixture.cgf())?;
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Sibling("skin.skinm") => Some(fixture.cgfm()),
            CompanionPath::Sibling("skin_lod1.skinm") => Some(opaque.clone()),
            _ => None,
        })
    };
    let model = Model::read_with_materials(
        "skin.skin",
        &root,
        &mut companions,
        LodSelection::All,
        PositionBox::default(),
        MaterialReading::Skipped,
    )?;
    assert_directory(&model, &expected, ModelFile::LodGeometry(1));
    assert!(
        model.lods.iter().all(|lod| lod.level != 1),
        "no fabricated geometry LOD"
    );
    assert!(
        model
            .chunks
            .iter()
            .all(|chunk| chunk.file != ModelFile::LodModel(1)),
        "unread level model has no invented physical fields"
    );
    Ok(())
}

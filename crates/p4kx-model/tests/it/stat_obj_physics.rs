// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_chunks::{ChunkFile, ivo};
use p4kx_model::{
    PhysicsBlob, PhysicsBlobError, PhysicsGeometry, StatObjPhysics, StatObjPhysicsError,
};
use p4kx_testkit::TestChunkFile;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const WHOLE_OBJECT: u32 = 0x0002_FFFF;

fn shape(kind: u32, words: &[f32]) -> Vec<u8> {
    let mut header = [0; 68];
    header[..4].copy_from_slice(&2_u32.to_le_bytes());
    let mut bytes = header.to_vec();
    bytes.extend(kind.to_le_bytes());
    bytes.extend(words.iter().flat_map(|value| value.to_le_bytes()));
    bytes
}

fn chunk(slots: &[(u32, &[u8])], tail: &[u8]) -> TestResult<Vec<u8>> {
    let mut header = [0; 80];
    header[40..44].copy_from_slice(&u32::try_from(slots.len())?.to_le_bytes());
    let mut bytes = header.to_vec();
    for (key, bytes_of_slot) in slots {
        bytes.extend(u32::try_from(bytes_of_slot.len())?.to_le_bytes());
        bytes.extend(key.to_le_bytes());
    }
    for (_, bytes_of_slot) in slots {
        bytes.extend_from_slice(bytes_of_slot);
    }
    bytes.extend_from_slice(tail);
    Ok(bytes)
}

#[test]
fn table_retains_duplicate_keys_whole_object_ranges_and_tail() -> TestResult {
    let first = shape(4, &[1.0, -0.0, 3.0, 4.0]);
    let second = 0xDEAD_BEB5_u32.to_le_bytes();
    let input = chunk(
        &[(WHOLE_OBJECT, &first), (WHOLE_OBJECT, &second)],
        &[9, 8, 7],
    )?;
    let physics = StatObjPhysics::read(&input)?;
    let slots: Vec<_> = physics.slots().collect();
    assert_eq!(slots.len(), 2);
    assert_eq!(slots[0].key(), WHOLE_OBJECT);
    assert_eq!(slots[1].key(), WHOLE_OBJECT);
    assert_eq!(slots[0].ordinal(), 0);
    assert_eq!(slots[1].ordinal(), 1);
    assert_eq!(slots[0].part(), 2);
    assert_eq!(slots[0].node(), None);
    assert_eq!(slots[0].length_range(), 80..84);
    assert_eq!(slots[0].key_range(), 84..88);
    assert_eq!(slots[0].declared_range(), 96..184);
    assert_eq!(slots[0].bytes(), Some(first.as_slice()));
    assert_eq!(physics.trailing_range(), Some(188..191));
    let PhysicsBlob::Geometry(PhysicsGeometry::Sphere(sphere)) = slots[0].blob() else {
        return Err("expected version-2 sphere".into());
    };
    assert_eq!(sphere.centre[1].bits(), (-0.0_f32).to_bits());
    assert_eq!(sphere.centre[1].range(), 76..80);
    assert_eq!(sphere.radius.value(), 4.0);
    assert!(matches!(slots[1].blob(), PhysicsBlob::SignedDistanceField));
    Ok(())
}

#[test]
fn independent_dispositions_keep_good_siblings() -> TestResult {
    let invalid = shape(4, &[f32::NAN, 0.0, 0.0, 1.0]);
    let unknown = shape(99, &[]);
    let damage = 0xCAFE_BABE_u32.to_le_bytes();
    let unsupported_version = 3_u32.to_le_bytes();
    let capsule = shape(6, &[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.5, 2.0]);
    let input = chunk(
        &[
            (1, &invalid),
            (9, &unknown),
            (37, &damage),
            (42, &unsupported_version),
            (0x0006_0123, &capsule),
        ],
        &[],
    )?;
    let physics = StatObjPhysics::read(&input)?;
    let slots: Vec<_> = physics.slots().collect();
    assert!(matches!(
        slots[0].blob(),
        PhysicsBlob::Malformed(PhysicsBlobError::NonFinite { offset: 72 })
    ));
    assert!(matches!(slots[1].blob(), PhysicsBlob::UnknownKind(99)));
    assert!(matches!(slots[2].blob(), PhysicsBlob::DamageMesh));
    assert!(matches!(slots[3].blob(), PhysicsBlob::UnknownMarker(3)));
    assert_eq!(slots[4].node(), Some(0x0123));
    assert_eq!(slots[4].part(), 6);
    let PhysicsBlob::Geometry(PhysicsGeometry::Capsule(shape)) = slots[4].blob() else {
        return Err("expected version-2 capsule".into());
    };
    assert_eq!(shape.half_height.value(), 2.0);
    assert_eq!(shape.axis[2].range(), 92..96);
    Ok(())
}

#[test]
fn truncated_blob_and_table_do_not_fabricate_original_bytes() -> TestResult {
    let mut input = chunk(&[(1, &[]), (2, &[])], &[7, 8, 9])?;
    input[80..84].copy_from_slice(&u32::MAX.to_le_bytes());
    let physics = StatObjPhysics::read(&input)?;
    let slots: Vec<_> = physics.slots().collect();
    assert_eq!(slots[0].declared_range(), 96..96 + u64::from(u32::MAX));
    assert_eq!(slots[0].bytes(), None);
    assert_eq!(slots[1].bytes(), None);
    assert!(matches!(slots[0].blob(), PhysicsBlob::MissingPayload));
    assert_eq!(physics.trailing_range(), None);
    assert!(StatObjPhysics::read(&input[..95]).is_err());
    input[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(StatObjPhysics::read(&input).is_err());
    Ok(())
}

fn triangle_mesh(index: u16) -> Vec<u8> {
    let mut blob = shape(1, &[]);
    for word in [3_u32, 1, 3, 0, 1] {
        blob.extend(word.to_le_bytes());
    }
    blob.extend([1, 0]);
    blob.extend(15_u16.to_le_bytes());
    blob.extend(16_u16.to_le_bytes());
    blob.push(7);
    for value in [0.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
        blob.extend(value.to_le_bytes());
    }
    for index in [0_u16, 1, index] {
        blob.extend(index.to_le_bytes());
    }
    blob.extend([91, 92, 93]);
    blob
}

#[test]
fn indexed_triangles_preserve_words_material_bytes_and_unread_tail() -> TestResult {
    let triangle = triangle_mesh(2);
    let invalid = triangle_mesh(3);
    let input = chunk(&[(1, &triangle), (2, &invalid)], &[])?;
    let physics = StatObjPhysics::read(&input)?;
    let slots: Vec<_> = physics.slots().collect();
    let PhysicsBlob::Geometry(PhysicsGeometry::Trimesh(mesh)) = slots[0].blob() else {
        return Err("expected version-2 triangle mesh".into());
    };
    assert_eq!(mesh.material_ids(), &[7]);
    assert_eq!(mesh.material_range(), 98..99);
    assert_eq!(mesh.vertex_range(), 99..135);
    assert_eq!(mesh.index_range(), 135..141);
    assert_eq!(mesh.triangles().collect::<Vec<_>>(), vec![[0, 1, 2]]);
    assert_eq!(mesh.vertices().len(), 3);
    let vertex = mesh.vertices().nth(1).ok_or("expected original vertex")?;
    assert_eq!(vertex[0].value(), 1.0);
    assert_eq!(vertex[0].range(), 111..115);
    assert_eq!(
        slots[0].bytes().and_then(|value| value.get(141..)),
        Some(&[91, 92, 93][..])
    );
    assert!(matches!(
        slots[1].blob(),
        PhysicsBlob::Malformed(PhysicsBlobError::Index {
            index: 3,
            offset: 139,
            vertices: 3
        })
    ));
    Ok(())
}

#[test]
fn only_measured_original_chunk_table_version_is_interpreted() -> TestResult {
    let sphere = shape(4, &[0.0, 0.0, 0.0, 1.0]);
    let data = chunk(&[(WHOLE_OBJECT, &sphere)], &[])?;
    let file = TestChunkFile::ivo()
        .chunk(ivo::STAT_OBJ_PHYSICS.id(), 0x901, data.as_slice())
        .chunk(ivo::STAT_OBJ_PHYSICS.id(), 0x900, data.as_slice())
        .bytes();
    let source = ChunkFile::parse(file)?;
    assert!(matches!(
        StatObjPhysics::read_chunk(&source, 0),
        Err(StatObjPhysicsError::UnsupportedVersion {
            index: 0,
            version: 0x901
        })
    ));
    assert_eq!(source.chunk_data(0), Some(data.as_slice()));
    assert_eq!(StatObjPhysics::read_chunk(&source, 1)?.slots().len(), 1);
    assert!(matches!(
        StatObjPhysics::read_chunk(&source, 2),
        Err(StatObjPhysicsError::NotPhysics { index: 2 })
    ));
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::{ChunkFile, ivo};
use p4kx_testkit::{TestChunkFile, TestModel, mesh_streams};

use crate::{
    CompanionError, CompanionPath, CompanionResolver, MeshChunkSource, MeshStreamOccurrence, Model,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const SUBMESH_AT: u64 = 168;

#[test]
fn physical_streams_match_an_independent_walk_for_both_index_widths() -> TestResult {
    for wide in [false, true] {
        let model = if wide {
            TestModel::cube().with_wide_indices()
        } else {
            TestModel::cube()
        };
        let body = model.skin_chunk();
        let expected = mesh_streams(&body).ok_or("independent original stream inventory")?;
        let wrapped = TestChunkFile::ivo().chunk(ivo::SKIN_2.id(), 0x900, &body[..]);
        let file = ChunkFile::parse(wrapped.bytes())?;
        let chunk = file.chunks().first().ok_or("original chunk")?;
        let bytes = file.chunk_data(0).ok_or("original chunk bytes")?;
        let layout = MeshChunkSource::read(chunk, bytes)?.ok_or("measured mesh layout")?;
        assert_eq!(
            layout.counts().vertices,
            expected.vertices,
            "physical vertex count"
        );
        assert_eq!(
            layout.counts().indices,
            expected.indices,
            "physical index count"
        );
        let rows = layout.streams().collect::<Result<Vec<_>, _>>()?;
        assert_eq!(
            rows.len(),
            expected.streams.len(),
            "count of original stream headers"
        );
        for (row, raw) in rows.into_iter().zip(expected.streams) {
            let MeshStreamOccurrence::Complete(source) = row else {
                return Err("complete stream".into());
            };
            let range = source.payload_range();
            let base = chunk.offset();
            assert_eq!(source.header().tag(), raw.tag, "exact raw tag");
            assert_eq!(
                source.header().stride(),
                raw.element_size,
                "exact raw element width"
            );
            assert_eq!(
                range,
                base + raw.data.start as u64..base + raw.data.end as u64,
                "exact original span"
            );
            assert_eq!(
                source.bytes(),
                body.get(raw.data).ok_or("independent stream bytes")?,
                "unchanged payload"
            );
        }
    }
    Ok(())
}

#[test]
fn original_index_words_and_page_base_reconcile_to_the_render_mesh() -> TestResult {
    let file = page_fixture()?;
    let chunk = file.chunks().first().ok_or("physical mesh chunk")?;
    let source = MeshChunkSource::read(chunk, file.chunk_data(0).ok_or("bounded mesh")?)?
        .ok_or("mesh source")?;
    let submesh = source
        .submeshes()
        .next()
        .ok_or("original first submesh row")?;
    assert_eq!(
        submesh.range().start,
        chunk.offset() + SUBMESH_AT,
        "original submesh position"
    );
    assert_eq!(submesh.material(), 97, "stored material is not dense");
    assert_eq!(
        submesh.node(),
        23,
        "stored skin zone is not interpreted node zero"
    );
    let indices = source
        .streams()
        .find_map(|item| match item {
            Ok(MeshStreamOccurrence::Complete(stream)) if stream.header().is_index_stream() => {
                Some(stream)
            }
            _ => None,
        })
        .ok_or("original index stream")?;
    let references = submesh
        .vertices(indices, source.counts())?
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        references
            .iter()
            .map(|row| row.source_vertex)
            .collect::<Vec<_>>(),
        [4, 5, 6],
        "page-base fixture"
    );
    assert_eq!(
        references.first().ok_or("first triangle index")?.range,
        indices.payload_range().start..indices.payload_range().start + 2,
        "u16 source word"
    );
    assert_renderer(&file)
}

fn assert_renderer(file: &ChunkFile<Vec<u8>>) -> TestResult {
    let model = Model::read("source.cgf", file, &mut MissingCompanions)?;
    let mesh = model
        .lods
        .first()
        .and_then(|level| level.meshes.first())
        .ok_or("read renderer mesh")?;
    assert_eq!(
        mesh.source.get(..3),
        Some(&[4_u32, 5, 6][..]),
        "the renderer's first-use source map"
    );
    assert_eq!(
        mesh.submeshes
            .first()
            .ok_or("read renderer submesh")?
            .node_id,
        23,
        "original submesh zone retained"
    );
    Ok(())
}

#[test]
fn unknown_tail_keeps_prior_streams_and_does_not_invent_payload() -> TestResult {
    let mut body = TestModel::cube().skin_chunk();
    let expected = mesh_streams(&body)
        .ok_or("original stream inventory")?
        .streams
        .len();
    body.extend(0xDEAD_CAFE_u32.to_le_bytes());
    body.extend(u32::MAX.to_le_bytes());
    let wrap = TestChunkFile::ivo().chunk(ivo::SKIN_2.id(), 0x900, body);
    let file = ChunkFile::parse(wrap.bytes())?;
    let chunk = file.chunks().first().ok_or("original chunk")?;
    let bytes = file.chunk_data(0).ok_or("original bytes")?;
    let layout = MeshChunkSource::read(chunk, bytes)?.ok_or("measured layout")?;
    let rows = layout.streams().collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        rows.len(),
        expected + 1,
        "tail occurrence remains addressable"
    );
    let Some(MeshStreamOccurrence::Incomplete(tail)) = rows.last() else {
        return Err("opaque overlong tail".into());
    };
    assert_eq!(tail.tag(), 0xDEAD_CAFE, "original unknown tag");
    assert_eq!(tail.stride(), u32::MAX, "original overlong size word");
    Ok(())
}

fn page_fixture() -> Result<ChunkFile<Vec<u8>>, Box<dyn std::error::Error>> {
    let mut body = TestModel::cube().skin_chunk();
    let start = usize::try_from(SUBMESH_AT)?;
    let first = body
        .get_mut(start..start + 48)
        .ok_or("first fixture submesh")?;
    first
        .get_mut(..2)
        .ok_or("material")?
        .copy_from_slice(&97_u16.to_le_bytes());
    first
        .get_mut(2..4)
        .ok_or("skin zone")?
        .copy_from_slice(&23_u16.to_le_bytes());
    first
        .get_mut(8..12)
        .ok_or("triangle count")?
        .copy_from_slice(&3_u32.to_le_bytes());
    first
        .get_mut(16..20)
        .ok_or("source page")?
        .copy_from_slice(&4_u32.to_le_bytes());
    let fixture = TestChunkFile::ivo().chunk(ivo::SKIN_2.id(), 0x900, body);
    Ok(ChunkFile::parse(fixture.bytes())?)
}

struct MissingCompanions;

impl CompanionResolver for MissingCompanions {
    fn resolve(&mut self, _path: CompanionPath<'_>) -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(None)
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::ops::Range;

use p4kx_extraction::{FieldOrigins, FieldPath, SourceUse, SourceUses};
use p4kx_gltf::MeshVertexMap;
use p4kx_model::MeshOrigin;

use crate::PackageWriteContext;

pub(crate) struct IndexCountOperand {
    pub(crate) mesh_origin: MeshOrigin,
    pub(crate) word: Range<u64>,
}

pub(crate) struct TriangleEvidence {
    triangles: u64,
}

impl TriangleEvidence {
    pub(crate) const fn new() -> Self {
        Self { triangles: 0 }
    }

    /// Checks each emitted count and visits its matching original operand when available.
    pub(crate) fn observe(
        &mut self,
        mesh: &MeshVertexMap<'_>,
        context: &PackageWriteContext<'_>,
        mut record: impl FnMut(IndexCountOperand) -> io::Result<()>,
    ) -> io::Result<()> {
        for primitive in mesh.primitives() {
            context.check().map_err(io::Error::other)?;
            let count = primitive
                .index_count
                .ok_or_else(|| io::Error::other("emitted primitive has no index count"))?;
            self.triangles = self.triangles.checked_add(count / 3).ok_or_else(|| {
                io::Error::other("emitted triangle count exceeds its representation")
            })?;
            if let (Some(mesh_origin), Some(original)) = (mesh.origin, primitive.original)
                && u64::from(original.index_count()) == count
            {
                record(IndexCountOperand {
                    mesh_origin,
                    word: original.index_count_range(),
                })?;
            }
        }
        Ok(())
    }

    pub(crate) fn verify(&self, written: usize) -> io::Result<()> {
        if usize::try_from(self.triangles).ok() != Some(written) {
            return Err(io::Error::other(
                "observed primitive counts disagree with the written triangle summary",
            ));
        }
        Ok(())
    }
}

/// Addresses the count at the package mesh slot using its known original operands.
pub(crate) fn append_field(
    slot: usize,
    fields: &mut FieldOrigins,
    sources: &SourceUses,
    contributions: &[SourceUse],
) -> io::Result<()> {
    let mut address = FieldPath::parse("/meshes").map_err(io::Error::other)?;
    address.push_index(slot).map_err(io::Error::other)?;
    address.push_field("triangles").map_err(io::Error::other)?;
    // Original model, level and visibility selection are outside this observation.
    let origin = sources.origin(contributions, false);
    fields.insert(address, origin).map_err(io::Error::other)
}

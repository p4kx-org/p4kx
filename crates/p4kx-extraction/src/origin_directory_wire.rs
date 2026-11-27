// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use crate::origin_companion::{self, OriginArtifactBinding, OriginCompanionRow};
use crate::{
    DecimalU64, FieldOrigin, OriginArtifactDigest, OriginArtifactName, OriginOutputAddress,
    SourceInventory,
};

pub(crate) struct CommittedArtifact {
    pub(crate) name: OriginArtifactName,
    pub(crate) digest: OriginArtifactDigest,
}

pub(crate) fn write(
    mut output: impl Write,
    artifacts: &[CommittedArtifact],
    inventory: &SourceInventory,
    fields: impl IntoIterator<Item = (usize, OriginOutputAddress, FieldOrigin)>,
) -> io::Result<()> {
    origin_companion::row(
        &mut output,
        &OriginCompanionRow::header(&OriginArtifactBinding::Directory),
    )?;
    for artifact in artifacts {
        origin_companion::row(
            &mut output,
            &OriginCompanionRow::Artifact {
                path: &artifact.name,
                digest: &artifact.digest,
            },
        )?;
    }
    origin_companion::row(
        &mut output,
        &OriginCompanionRow::ArtifactTableEnd {
            artifacts: DecimalU64::new(artifacts.len() as u64),
        },
    )?;
    origin_companion::write_inventory(&mut output, inventory)?;
    let mut count = 0_u64;
    let mut addressed = vec![false; artifacts.len()];
    for (artifact, address, origin) in fields {
        *addressed.get_mut(artifact).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "field addresses an uncommitted origin artifact",
            )
        })? = true;
        origin_companion::row(
            &mut output,
            &OriginCompanionRow::ArtifactField {
                artifact: DecimalU64::new(artifact as u64),
                address: &address,
                origin: &origin,
            },
        )?;
        count = count
            .checked_add(1)
            .ok_or_else(|| io::Error::other("origin field count exceeds u64"))?;
    }
    if addressed.contains(&false) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a committed artifact has no origin fields",
        ));
    }
    origin_companion::write_end(&mut output, inventory, count)
}

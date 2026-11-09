// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;

use p4kx_cryxml::Xml;

use crate::mtl_source::{MaterialEncoding, MaterialSourceLimits};
use crate::mtl_source_error::MaterialReadError;
use crate::mtl_xml::XmlElement;
use crate::{
    MaterialError, MaterialLibrary, MaterialReadObservation, mtl_parse, mtl_source_capture,
};

const MAX_SOURCE_DEPTH: usize = 64;

pub(crate) fn parse(path: &str, bytes: &[u8]) -> Result<MaterialLibrary, MaterialError> {
    let limits = MaterialSourceLimits {
        source_bytes: bytes.len().max(1),
        ..MaterialSourceLimits::default()
    };
    match read(path, bytes, limits, || Ok::<(), Infallible>(())) {
        Ok(library) => Ok(library),
        Err(MaterialReadError::Material(error)) => Err(error),
        Err(MaterialReadError::Interrupted(never)) => match never {},
    }
}

pub(crate) fn read<Interrupted>(
    path: &str,
    bytes: &[u8],
    limits: MaterialSourceLimits,
    checkpoint: impl FnMut() -> Result<(), Interrupted>,
) -> Result<MaterialLibrary, MaterialReadError<Interrupted>> {
    read_observed(path, bytes, limits, checkpoint, |_| Ok(()))
}

pub(crate) fn read_observed<Interrupted>(
    path: &str,
    bytes: &[u8],
    limits: MaterialSourceLimits,
    mut checkpoint: impl FnMut() -> Result<(), Interrupted>,
    mut observer: impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<MaterialLibrary, MaterialReadError<Interrupted>> {
    if limits.source_bytes == 0
        || limits.elements == 0
        || limits.text_bytes == 0
        || limits.depth > MAX_SOURCE_DEPTH
    {
        return Err(MaterialError::InvalidSourceLimits.into());
    }
    checkpoint().map_err(MaterialReadError::Interrupted)?;
    if bytes.len() > limits.source_bytes {
        return Err(MaterialError::SourceBytes {
            limit: limits.source_bytes,
        }
        .into());
    }
    let xml = p4kx_cryxml::read(bytes).map_err(MaterialError::from)?;
    checkpoint().map_err(MaterialReadError::Interrupted)?;
    match xml {
        Xml::CryXml(document) => project(
            path,
            document.root(),
            MaterialEncoding::CryXml,
            document.is_lossy(),
            limits,
            &mut checkpoint,
            &mut observer,
        ),
        Xml::Text(text) => {
            let text = str::from_utf8(text).map_err(MaterialError::from)?;
            p4kx_cryxml::check_text_depth(text).map_err(MaterialError::from)?;
            let document = roxmltree::Document::parse(text).map_err(MaterialError::from)?;
            checkpoint().map_err(MaterialReadError::Interrupted)?;
            project(
                path,
                document.root_element(),
                MaterialEncoding::Text,
                false,
                limits,
                &mut checkpoint,
                &mut observer,
            )
        }
    }
}

fn project<'source, Interrupted>(
    path: &str,
    root: impl XmlElement<'source>,
    encoding: MaterialEncoding,
    lossy: bool,
    limits: MaterialSourceLimits,
    checkpoint: &mut impl FnMut() -> Result<(), Interrupted>,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<MaterialLibrary, MaterialReadError<Interrupted>> {
    let authored = mtl_source_capture::capture(root, encoding, lossy, limits, checkpoint)?;
    let library = mtl_parse::library(path, root, authored, checkpoint, observer)?;
    checkpoint().map_err(MaterialReadError::Interrupted)?;
    Ok(library)
}

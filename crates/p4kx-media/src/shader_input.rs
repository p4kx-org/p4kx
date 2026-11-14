// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::io::Read as _;
use std::path::PathBuf;

use p4kx_extraction::{Archive, Entry};

use crate::directory_index::DirectoryIndex;
use crate::directory_inputs::DirectoryInputs;
use crate::directory_lookup::DirectoryLookup;
use crate::shader_flags::{self, ShaderRead};
use crate::shader_index::ShaderTarget;
use crate::shader_lookup::ShaderLookup;
use crate::{
    ContainerIdentity, Identity, InputReceipt, MediaError, MediaWriter, RequiredInput, Subresource,
};
use crate::{required_input, source, traversal_container};

pub(crate) fn read(
    lookup: &ShaderLookup<'_>,
    target: &ShaderTarget,
    use_at: &Subresource,
    writer: &mut MediaWriter<'_>,
) -> Result<ShaderRead, MediaError> {
    if !lookup
        .archive
        .input_origin()
        .is_same_input(target.origin.input_origin())
    {
        return Err(MediaError::Contract(
            "shader target belongs to another observed Archive input".into(),
        ));
    }
    let mut ancestors = Vec::new();
    for ancestor in target.origin.ancestors() {
        let ancestor = ancestor.ok_or_else(|| {
            MediaError::Contract("shader target has unproved container ancestry".into())
        })?;
        ancestors.push(ancestor.index());
    }
    let request = Request {
        target,
        use_at,
        ancestors: &ancestors,
    };
    descend(lookup, lookup.archive, &request, 0, &mut Vec::new(), writer)
}

struct Request<'a> {
    target: &'a ShaderTarget,
    use_at: &'a Subresource,
    ancestors: &'a [usize],
}

fn descend(
    lookup: &ShaderLookup<'_>,
    archive: &Archive,
    request: &Request<'_>,
    depth: usize,
    chain: &mut Vec<ContainerIdentity>,
    writer: &mut MediaWriter<'_>,
) -> Result<ShaderRead, MediaError> {
    writer.checkpoint()?;
    if let Some(&position) = request.ancestors.get(depth) {
        let entry = archive
            .entries()
            .get(position)
            .ok_or_else(|| MediaError::Contract("shader container position disappeared".into()))?;
        let child =
            traversal_container::open(archive, entry, chain, lookup.root, lookup.budget, writer)?;
        chain.push(source::identity(archive, entry));
        let result = descend(lookup, &child, request, depth + 1, chain, writer);
        let _parent = chain.pop();
        return result;
    }
    let entry = verified_entry(archive, request.target)?;
    if entry.uncompressed_size() > crate::shader_values::count(lookup.limits.source_bytes)? {
        return Ok(unsupported(None));
    }
    let keys = Cell::new(0);
    let index = DirectoryIndex::new(archive, &keys);
    let inputs = DirectoryInputs::try_new(archive.entries().len(), lookup.budget, writer)?;
    let directory =
        DirectoryLookup::new_with_inputs(&index, lookup.root, chain, lookup.selection, &inputs);
    required_input::with_reference_input(
        &directory,
        entry,
        request.use_at,
        writer,
        |input, writer| parse_input(input, lookup, writer),
    )
}

fn verified_entry<'a>(
    archive: &'a Archive,
    target: &ShaderTarget,
) -> Result<&'a Entry, MediaError> {
    if !archive.owns_origin(&target.origin) {
        return Err(MediaError::Contract(
            "shader target is outside its observed owning view".into(),
        ));
    }
    let entry = archive
        .entries()
        .get(target.origin.entry().index())
        .ok_or_else(|| MediaError::Contract("shader Entry position disappeared".into()))?;
    if entry.path() != target.origin.entry().path()
        || entry.metadata() != target.origin.entry().metadata()
    {
        return Err(MediaError::Contract(
            "shader target metadata differs from its observed input".into(),
        ));
    }
    Ok(entry)
}

fn parse_input(
    input: &mut RequiredInput<'_>,
    lookup: &ShaderLookup<'_>,
    writer: &MediaWriter<'_>,
) -> Result<ShaderRead, MediaError> {
    let path = match &input.receipt().source.identity {
        Identity::Entry { entry, .. } => PathBuf::from(&entry.path),
        Identity::Root => {
            return Err(MediaError::Contract(
                "shader target has no Entry identity".into(),
            ));
        }
    };
    let length = usize::try_from(input.receipt().source.size)
        .map_err(|_| MediaError::Contract("shader source length exceeds address space".into()))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|cause| MediaError::Io {
            path: path.clone(),
            source: std::io::Error::new(std::io::ErrorKind::OutOfMemory, cause),
        })?;
    bytes.resize(length, 0);
    input
        .read_exact(&mut bytes)
        .map_err(|source| MediaError::Io { path, source })?;
    match p4kx_shader::read_with(&bytes, lookup.limits, || writer.checkpoint()) {
        Ok(definition) => {
            shader_flags::project(&definition, input.receipt().clone(), lookup, writer)
        }
        Err(p4kx_shader::ReadError::Syntax(_)) => Ok(unsupported(Some(input.receipt().clone()))),
        Err(p4kx_shader::ReadError::Interrupted(error)) => Err(error),
        Err(other) => Err(MediaError::Contract(format!(
            "unsupported shader reader result: {other}"
        ))),
    }
}

const fn unsupported(receipt: Option<InputReceipt>) -> ShaderRead {
    ShaderRead {
        receipt,
        properties: Vec::new(),
        supported: false,
        unknown_names: true,
        common_flags: false,
    }
}

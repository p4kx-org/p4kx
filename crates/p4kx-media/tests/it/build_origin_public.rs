// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_media::{
    ArchiveProvider, Capability, MediaError, MediaWriter, Projection, Provider, ProviderRegistry,
    RootSource, SourceContext,
};
use serde_json::Value;

use super::build_origin_fixture::{self as fixture, Channel, Input, TestResult};
use super::build_origin_support as check;

#[derive(Clone, Copy)]
enum Mode {
    Modified,
    Deserialized,
    OriginalClone,
}

struct RootDelegate(Mode);

impl Provider for RootDelegate {
    fn capability(&self) -> Capability {
        ArchiveProvider.capability()
    }

    fn root(&self, root: &RootSource, writer: &mut MediaWriter<'_>) -> Result<(), MediaError> {
        let mut root = if matches!(self.0, Mode::Deserialized) {
            serde_json::from_value(serde_json::to_value(root)?)?
        } else {
            root.clone()
        };
        if matches!(self.0, Mode::Modified) {
            root.build
                .as_mut()
                .ok_or_else(|| MediaError::Contract("synthetic present Build".into()))?
                .branch = "forged".into();
        }
        ArchiveProvider.root(&root, writer)
    }

    fn source(
        &self,
        _context: &SourceContext<'_>,
        _writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        Ok(Projection::Skipped)
    }
}

#[test]
fn public_modified_and_deserialized_root_values_cannot_restore_original_build_authority()
-> TestResult {
    let bytes = fixture::manifest("owned")?;
    let channel = Channel::new(Input::File(&bytes), false)?;
    for (mode, name) in [
        (Mode::Modified, "modified"),
        (Mode::Deserialized, "decoded"),
        (Mode::OriginalClone, "original"),
    ] {
        let registry = ProviderRegistry::try_new(vec![Box::new(RootDelegate(mode))])?;
        let run = channel.export(name, &fixture::options(), &registry)?;
        let expected = if matches!(mode, Mode::Modified) {
            "forged"
        } else {
            "owned"
        };
        assert_eq!(
            run.root()?
                .1
                .pointer("/data/build/branch")
                .and_then(Value::as_str),
            Some(expected),
            "public payload behavior remains unchanged"
        );
        if matches!(mode, Mode::OriginalClone) {
            check::file_field(&run, "records", "/data/build/branch", Some("Data.Branch"))?;
        } else {
            check::unknown(&run, "records", "/data/build/branch")?;
        }
    }
    Ok(())
}

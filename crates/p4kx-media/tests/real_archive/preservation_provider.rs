// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_media::{
    Capability, FieldGroup, MediaError, MediaKind, MediaWriter, Projection, Provider, SourceContext,
};

pub(super) struct PreservationProvider;

impl Provider for PreservationProvider {
    fn capability(&self) -> Capability {
        let mut capability = Capability::new(
            MediaKind::Archive,
            "verification.selected-raw",
            "1.0.0",
            "centrally retained exact selected decoded Entry bytes; no typed metadata projection",
        );
        capability
            .field_groups
            .push(FieldGroup::new_with_unsupported_projection(
                "archive-layout",
                "directory metadata and domain projection are outside this raw-preservation probe",
            ));
        capability
    }

    fn source(
        &self,
        _context: &SourceContext<'_>,
        _writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        Ok(Projection::Skipped)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use p4kx_extraction::{Archive, Filter, MetadataSection};
    use p4kx_media::{ArchiveProvider, MediaError, ProviderRegistry};
    use p4kx_testkit::{ArchiveBuilder, TestEntry};

    use crate::TestResult;

    #[test]
    fn selected_raw_probe_stays_bounded_when_unselected_directory_exceeds_payload_budget()
    -> TestResult {
        let folder = tempfile::tempdir()?;
        let archive = large_directory(folder.path())?;
        let range = archive
            .metadata_view()
            .span(MetadataSection::Directory)?
            .ok_or("fixture directory span")?
            .range();
        assert!(
            range.len() > crate::MAX_SOURCE_BYTES,
            "actual directory exceeds probe read cap"
        );
        let output = folder.path().join("default-archive");
        let providers = ProviderRegistry::try_new(vec![Box::new(ArchiveProvider)])?;
        let failure = p4kx_media::export_with(
            &archive,
            &output,
            &crate::options(Filter::parse("selected.bin")?)?,
            &providers,
        )
        .err()
        .ok_or("full Archive metadata unexpectedly fits selected-payload budget")?;
        assert!(
            source_limit(&failure),
            "full provider retains the original source-bytes limit cause"
        );
        assert!(
            !output.join("media-index.json").exists(),
            "failed metadata is not complete"
        );
        crate::verify_archive(&archive, Filter::parse("selected.bin")?)?;
        Ok(())
    }

    fn large_directory(root: &Path) -> TestResult<Archive> {
        const NAME_BYTES: usize = 60 << 10;
        let suffix = "x".repeat(NAME_BYTES);
        let count = usize::try_from(crate::MAX_SOURCE_BYTES)?.div_ceil(NAME_BYTES);
        let mut builder = ArchiveBuilder::new().entry(TestEntry::stored("selected.bin", b"x"));
        for index in 0..count {
            builder = builder.entry(TestEntry::stored(format!("{index}/{suffix}"), b""));
        }
        let path = root.join("synthetic.p4k");
        builder.write_to(&path)?;
        Ok(Archive::open(path)?)
    }

    fn source_limit(failure: &MediaError) -> bool {
        use std::error::Error;

        let mut error: Option<&(dyn Error + 'static)> = Some(failure);
        while let Some(cause) = error {
            if matches!(
                cause.downcast_ref::<MediaError>(),
                Some(MediaError::Limit { budget: "source_bytes", limit })
                if *limit == crate::MAX_SOURCE_BYTES
            ) {
                return true;
            }
            error = cause.source();
        }
        false
    }
}

"""Checks source and native static-link inventory boundaries without Linux execution."""

import hashlib
import importlib.util
import io
from pathlib import Path
import shutil
import tarfile
import unittest
from unittest.mock import patch

specification = importlib.util.spec_from_file_location("collect_sources", Path(__file__).with_name("collect-sources.py"))
collect_sources = importlib.util.module_from_spec(specification)
specification.loader.exec_module(collect_sources)


class SourceInventoryTests(unittest.TestCase):
    """Uses only synthetic files in the explicitly supplied scratch directory."""

    root = None

    def setUp(self):
        """Creates an isolated case directory below the caller's scratch root."""
        self.directory = self.root / self.id().rsplit(".", 1)[-1]
        self.directory.mkdir()

    def test_original_notices_keep_paths_and_digests(self):
        """Original notices remain byte-exact and retain their source-relative paths."""
        source = self.directory / "source"
        (source / "nested").mkdir(parents=True)
        original = b"Synthetic original licence\n"
        (source / "nested" / "COPYING").write_bytes(original)
        (source / "unrelated").write_text("not a notice")
        destination = self.directory / "notices"
        records = collect_sources.copy_licences(source, destination)
        self.assertEqual(records, [{"path": str(Path("nested") / "COPYING"), "sha256": hashlib.sha256(original).hexdigest()}])
        self.assertEqual((destination / "nested" / "COPYING").read_bytes(), original)

    def test_missing_notice_is_not_silently_accepted(self):
        """A source tree without original legal evidence fails collection."""
        with self.assertRaisesRegex(ValueError, "no original licence"):
            collect_sources.copy_licences(self.directory, self.directory / "notices")

    def test_download_rejects_transport_and_changed_source(self):
        """Source transport and pin mismatch are enforced before inclusion."""
        destination = self.directory / "source.tar"
        with self.assertRaisesRegex(ValueError, "HTTPS"):
            collect_sources.download("http://example.invalid/source", destination)
        with patch.object(collect_sources.urllib.request, "urlopen", return_value=io.BytesIO(b"changed")):
            with self.assertRaisesRegex(ValueError, "digest mismatch"):
                collect_sources.download("https://example.invalid/source", destination, "0" * 64)

    def test_static_archive_requires_locked_owner(self):
        """An archive outside the pinned source/APK closure prevents release."""
        library = self.directory / "libcompiler.a"
        library.write_bytes(b"synthetic archive")
        (self.directory / "runtime-link.map").write_text(f"LOAD {library.as_posix()}\n")
        installed = [{"P": "gcc", "V": "15.2.0-r2", "o": "gcc"}]
        response = collect_sources.subprocess.CompletedProcess([], 0, f"{library} is owned by gcc-15.2.0-r2\n")
        with patch.object(collect_sources.subprocess, "run", return_value=response):
            result = collect_sources.linked_archives(self.directory, installed, {"gcc": {}})
            self.assertEqual(result[0]["source"], "gcc")
            self.assertEqual(result[0]["sha256"], hashlib.sha256(library.read_bytes()).hexdigest())
            with self.assertRaisesRegex(ValueError, "unlocked static archive"):
                collect_sources.linked_archives(self.directory, installed, {})

    def test_empty_link_map_cannot_claim_static_coverage(self):
        """A missing static closure is rejected rather than recorded as checked."""
        (self.directory / "runtime-link.map").write_text("no linked archive evidence\n")
        with self.assertRaisesRegex(ValueError, "no static link map"):
            collect_sources.linked_archives(self.directory, [], {})

    def test_source_archive_rejects_parent_traversal(self):
        """A source archive cannot escape its corresponding-source directory."""
        sources = self.directory / "sources"
        sources.mkdir()
        malicious = self.directory / "malicious.tar"
        with tarfile.open(malicious, "w") as archive:
            entry = tarfile.TarInfo("../../outside")
            entry.size = 1
            archive.addfile(entry, io.BytesIO(b"x"))
        def local_download(url, destination, expected):
            shutil.copyfile(malicious, destination)
            return "a" * 64
        dependency = {"name": "synthetic", "url": "https://example.invalid/source", "sha256": "a" * 64}
        with patch.object(collect_sources, "download", side_effect=local_download):
            with self.assertRaises(tarfile.OutsideDestinationError):
                collect_sources.archive_source(dependency, sources, self.directory / "notices")
        self.assertFalse((self.directory / "outside").exists())


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--directory", required=True, type=Path)
    arguments, remaining = parser.parse_known_args()
    SourceInventoryTests.root = arguments.directory.resolve()
    SourceInventoryTests.root.mkdir()
    unittest.main(argv=[__file__, *remaining])

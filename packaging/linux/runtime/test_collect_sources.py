"""Checks source and native static-link inventory boundaries without Linux execution."""

import hashlib
import importlib.util
import io
from pathlib import Path
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

    def test_transient_download_failure_retries_the_same_pinned_source(self):
        """A bounded retry keeps the original source URL and verifies its pinned digest."""
        destination = self.directory / "source.tar"
        payload = b"synthetic pinned source"
        expected = hashlib.sha256(payload).hexdigest()
        responses = [collect_sources.urllib.error.URLError("synthetic unreachable"), io.BytesIO(payload)]
        with patch.object(collect_sources.urllib.request, "urlopen", side_effect=responses) as request:
            with patch.object(collect_sources.time, "sleep") as sleep:
                actual = collect_sources.download("https://example.invalid/source", destination, expected)
        self.assertEqual(actual, expected)
        self.assertEqual(destination.read_bytes(), payload)
        self.assertEqual(request.call_count, 2)
        self.assertEqual(request.call_args_list[0], request.call_args_list[1])
        sleep.assert_called_once_with(1)

    def test_download_retry_exhaustion_preserves_the_original_network_error(self):
        """Four failed attempts stop collection without treating incomplete source as verified."""
        failure = collect_sources.urllib.error.URLError("synthetic unreachable")
        with patch.object(collect_sources.urllib.request, "urlopen", side_effect=failure) as request:
            with patch.object(collect_sources.time, "sleep") as sleep:
                with self.assertRaises(collect_sources.urllib.error.URLError) as caught:
                    collect_sources.download("https://example.invalid/source", self.directory / "source.tar")
        self.assertIs(caught.exception, failure)
        self.assertEqual(request.call_count, 4)
        self.assertEqual([call.args[0] for call in sleep.call_args_list], [1, 2, 4])
        self.assertFalse((self.directory / "source.tar").exists())

    def test_missing_or_changed_source_is_not_retried(self):
        """Permanent HTTP failures and digest drift fail without an alternate source or retry."""
        missing = collect_sources.urllib.error.HTTPError("https://example.invalid/source", 404, "missing", None, None)
        cases = [(missing, collect_sources.urllib.error.HTTPError), (io.BytesIO(b"changed"), ValueError)]
        for response, failure_type in cases:
            with self.subTest(failure=failure_type.__name__):
                effect = {"side_effect": response} if isinstance(response, Exception) else {"return_value": response}
                with patch.object(collect_sources.urllib.request, "urlopen", **effect) as request:
                    with patch.object(collect_sources.time, "sleep") as sleep:
                        with self.assertRaises(failure_type):
                            collect_sources.download("https://example.invalid/source", self.directory / "source.tar", "0" * 64)
                request.assert_called_once()
                sleep.assert_not_called()
    def test_static_archive_requires_locked_owner(self):
        """An archive outside the pinned source/APK closure prevents release."""
        library = self.directory / "libcompiler.a"
        library.write_bytes(b"synthetic archive")
        (self.directory / "runtime-link.map").write_text(f"LOAD {library.as_posix()}\n")
        installed = [{"P": "gcc", "V": "15.2.0-r2", "o": "gcc"}]
        response = collect_sources.subprocess.CompletedProcess([], 0, f"{library} is owned by gcc-15.2.0-r2\n")
        with patch.object(collect_sources.subprocess, "run", return_value=response):
            result = collect_sources.linked_archives(self.directory, installed, {"gcc": {}}, {"archives": []})
            self.assertEqual(result[0]["source"], "gcc")
            self.assertEqual(result[0]["sha256"], hashlib.sha256(library.read_bytes()).hexdigest())
            with self.assertRaisesRegex(ValueError, "unlocked static archive"):
                collect_sources.linked_archives(self.directory, installed, {}, {"archives": []})

    def test_empty_link_map_cannot_claim_static_coverage(self):
        """A missing static closure is rejected rather than recorded as checked."""
        (self.directory / "runtime-link.map").write_text("no linked archive evidence\n")
        with self.assertRaisesRegex(ValueError, "no static link map"):
            collect_sources.linked_archives(self.directory, [], {}, {"archives": []})

    def test_recipe_archive_requires_the_exact_official_commit_and_digest(self):
        """A recipe cannot change host, source commit, root or digest spelling."""
        commit = "a" * 40
        reference = {"url": f"https://github.com/alpinelinux/aports/archive/{commit}.tar.gz", "archive_root": f"aports-{commit}", "sha256": "b" * 64}
        self.assertEqual(collect_sources.recipe_source(commit, {commit: reference}), reference)
        for changed in (
            dict(reference, url="https://example.invalid/archive.tar.gz"),
            dict(reference, url=reference["url"].replace(commit, "c" * 40)),
            dict(reference, archive_root="unrelated-root"),
            dict(reference, sha256="not a digest"),
        ):
            with self.assertRaises(ValueError):
                collect_sources.recipe_source(commit, {commit: changed})

    def test_pinned_recipe_source_is_verified_before_abuild_unpack(self):
        """The matching source archive is hash-checked and the original notices collected."""
        commit = "a" * 40
        root = f"aports-{commit}-{commit}"
        stream = io.BytesIO()
        with tarfile.open(fileobj=stream, mode="w") as archive:
            data = b"pkgname=synthetic\npkgver=1.0\npkgrel=0\n"
            entry = tarfile.TarInfo(f"{root}/main/synthetic/APKBUILD")
            entry.size = len(data)
            archive.addfile(entry, io.BytesIO(data))
        payload = stream.getvalue()
        digest = hashlib.sha256(payload).hexdigest()
        sources = self.directory / "sources"
        sources.mkdir()
        reference = {"url": f"https://gitlab.alpinelinux.org/api/v4/projects/alpine%2Faports/repository/archive.tar.gz?sha={commit}", "archive_root": root, "sha256": digest}
        packages = [{"o": "synthetic", "V": "1.0-r0", "c": commit, "L": "MIT"}]
        def unpack(command, *, cwd, env, check):
            self.assertEqual(command, ["abuild", "-F", "fetch", "verify", "unpack"])
            self.assertTrue(check)
            self.assertTrue((cwd / "APKBUILD").is_file())
            self.assertEqual(env["SRCDEST"], str(sources / "synthetic" / "distfiles"))
            source = cwd / "src"
            source.mkdir()
            (source / "LICENSE").write_text("Synthetic original licence\n")
        with patch.object(collect_sources.urllib.request, "urlopen", return_value=io.BytesIO(payload)):
            with patch.object(collect_sources.subprocess, "run", side_effect=unpack) as abuild:
                result = collect_sources.alpine_source("synthetic", packages, sources, self.directory / "notices", {commit: reference})
        abuild.assert_called_once()
        self.assertEqual(result["recipe_archive_sha256"], digest)
        self.assertEqual(result["recipe_commit"], commit)
        self.assertEqual(len(result["licences"]), 1)
        changed_sources = self.directory / "changed-sources"
        changed_sources.mkdir()
        with patch.object(collect_sources.urllib.request, "urlopen", return_value=io.BytesIO(payload)):
            with patch.object(collect_sources.subprocess, "run") as abuild:
                with self.assertRaisesRegex(ValueError, "digest mismatch"):
                    collect_sources.alpine_source("synthetic", packages, changed_sources, self.directory / "changed-notices", {commit: dict(reference, sha256="0" * 64)})
        abuild.assert_not_called()

    def test_retained_build_input_rejects_parent_traversal(self):
        """A compiled-input receipt cannot read files outside the retained build root."""
        owned = self.directory / "owned"
        owned.mkdir()
        (self.directory / "outside").write_text("unowned input")
        with self.assertRaisesRegex(ValueError, "escapes its root"):
            collect_sources.build_inputs.inside(owned, "../outside")


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--directory", required=True, type=Path)
    arguments, remaining = parser.parse_known_args()
    SourceInventoryTests.root = arguments.directory.resolve()
    SourceInventoryTests.root.mkdir()
    unittest.main(argv=[__file__, *remaining])

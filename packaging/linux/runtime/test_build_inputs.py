"""Checks actual build-input path and digest binding using synthetic native build layouts."""

import argparse
import json
from pathlib import Path
import unittest

import build_inputs


class BuildInputTests(unittest.TestCase):
    """Exercises owned source/build/install paths without executing compilers or Linux programs."""

    root = None

    def setUp(self):
        """Creates a case directory below the explicit caller-owned scratch root."""
        self.directory = self.root / self.id().rsplit(".", 1)[-1]
        self.directory.mkdir()

    def fixture(self, architecture="x86_64"):
        """Creates matching synthetic source inputs, build archives and installed native paths."""
        root = self.directory / architecture / "dependency-build"
        root.mkdir(parents=True)
        script = root.parent / "installer.sh"
        script.write_text("synthetic verified recipe\n")
        (root / "upstream-install-dependencies.sh").write_bytes(script.read_bytes())
        receipt = {"original_recipe_sha256": build_inputs.digest(script), "retained_recipe_sha256": build_inputs.digest(script), "retained_recipe_path": str(script.resolve())}
        (root / "recipe.json").write_text(json.dumps(receipt))
        dependencies = []
        layouts = {}
        for name, paths in build_inputs.BUILT_ARCHIVES.items():
            source = f"{name}-1.0"
            archive = root / f"{name}.tar.gz"
            archive.write_bytes(f"synthetic original {name} source".encode())
            dependencies.append({"name": name, "version": "1.0", "url": f"https://example.invalid/{archive.name}", "sha256": build_inputs.digest(archive), "archive_root": source})
            actual = []
            for relative, installed in paths:
                compiled = root / source / relative
                compiled.parent.mkdir(parents=True, exist_ok=True)
                compiled.write_bytes(f"{architecture}:{name}:{relative}".encode())
                destination = root.parent / "installed" / Path(installed).name
                destination.parent.mkdir(exist_ok=True)
                destination.write_bytes(compiled.read_bytes())
                actual.append((relative, str(destination)))
            layouts[name] = actual
        inputs = {"source_dependencies": dependencies}
        return inputs, root, layouts, build_inputs.record(inputs, root, layouts)

    def test_both_native_cpu_layouts_bind_installed_archives_to_actual_builds(self):
        """Each supported CPU retains all three exact compiled/installed path and hash pairs."""
        for architecture in ("x86_64", "aarch64"):
            with self.subTest(architecture=architecture):
                inputs, root, _, manifest = self.fixture(architecture)
                self.assertEqual(build_inputs.validate(inputs, root), manifest)
                self.assertEqual(len(manifest["archives"]), 3)
                for archive in manifest["archives"]:
                    self.assertEqual(build_inputs.bound_archive(Path(archive["installed_path"]), manifest), archive)

    def test_an_identical_basename_outside_the_recorded_install_path_is_rejected(self):
        """Byte-identical copies at unowned paths cannot borrow another source's provenance."""
        _, _, _, manifest = self.fixture()
        original = Path(manifest["archives"][0]["installed_path"])
        outsider = self.directory / original.name
        outsider.write_bytes(original.read_bytes())
        with self.assertRaisesRegex(ValueError, "unowned path"):
            build_inputs.bound_archive(outsider, manifest)

    def test_mutated_installed_archives_are_rejected_before_and_after_link(self):
        """Neither an altered installation nor a stale pre-link receipt can certify input bytes."""
        inputs, root, layouts, manifest = self.fixture()
        installed = Path(manifest["archives"][0]["installed_path"])
        installed.write_bytes(b"different static library")
        with self.assertRaisesRegex(ValueError, "pre-link build receipt"):
            build_inputs.bound_archive(installed, manifest)
        with self.assertRaisesRegex(ValueError, "actual build output"):
            build_inputs.record(inputs, root, layouts)

    def test_mutated_build_archive_cannot_be_hidden_by_an_intact_installed_copy(self):
        """The retained build artifact itself must match the bytes the linker received."""
        inputs, root, _, manifest = self.fixture()
        (root / manifest["archives"][0]["build_archive"]).write_bytes(b"different build archive")
        with self.assertRaisesRegex(ValueError, "compiled dependency archive changed"):
            build_inputs.validate(inputs, root)

    def test_swapped_verified_source_inputs_do_not_retain_the_old_origin(self):
        """A different retained source archive invalidates the dependency's source identity."""
        inputs, root, _, manifest = self.fixture()
        sources = list(manifest["sources"].values())
        (root / sources[0]["archive"]).write_bytes((root / sources[1]["archive"]).read_bytes())
        with self.assertRaisesRegex(ValueError, "source digest mismatch"):
            build_inputs.validate(inputs, root)

    def test_a_changed_executed_recipe_invalidates_the_build_receipt(self):
        """Recorded library hashes do not conceal a different executed dependency recipe."""
        inputs, root, _, manifest = self.fixture()
        Path(manifest["recipe"]["retained_recipe_path"]).write_text("different recipe\n")
        with self.assertRaisesRegex(ValueError, "actual dependency build recipe changed"):
            build_inputs.validate(inputs, root)

    def test_workspace_retention_preserves_upstream_checks_and_native_build_commands(self):
        """Only workspace creation/cleanup changes in the verified upstream installer."""
        inputs, _, _, _ = self.fixture()
        upstream = self.directory / "upstream"
        script = upstream / "scripts/common/install-dependencies.sh"
        script.parent.mkdir(parents=True)
        prefix = '#!/bin/bash\ntmpdir="$(mktemp -d)"\n\n# clean it up whenever the script exits\ncleanup() {\n    rm -rf "$tmpdir"\n}\ntrap cleanup EXIT\n'
        commands = "# native build commands\n" + "\n".join(f"wget {item['url']}\necho '{item['sha256']}  input.tar' | sha256sum -c -" for item in inputs["source_dependencies"]) + "\nmake install\n"
        original = (prefix + commands).encode()
        script.write_bytes(original)
        retained = self.directory / "retained"
        build_inputs.prepare(upstream, inputs, retained)
        self.assertTrue(script.read_bytes().endswith(commands.encode()))
        self.assertEqual((retained / "upstream-install-dependencies.sh").read_bytes(), original)
        receipt = json.loads((retained / "recipe.json").read_text())
        build_inputs.validate_recipe(retained, receipt)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--directory", required=True, type=Path)
    arguments, remaining = parser.parse_known_args()
    BuildInputTests.root = arguments.directory.resolve()
    BuildInputTests.root.mkdir()
    unittest.main(argv=[__file__, *remaining])

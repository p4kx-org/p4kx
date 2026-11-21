# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Validate physical source uses and their immediate field markers."""

from typing import Any

from .manifest_fields import PackageError, read_array, read_object

_ENCRYPTION = ("encrypted", "unencrypted", "unknown")
_ROLES = ("payload", "selector", "conversion", "attempted", "reference")
_CONTRIBUTORS = ("payload", "selector", "conversion")
_SCOPES = ("payload", "archiveDirectoryMetadata", "fileMetadata")


class SourceEvidence:
    """Checks document-local source uses before validating field references."""

    def __init__(self, label: str) -> None:
        self.label = label
        self.sources: dict[str, tuple[str, ...]] = {}
        self.uses: dict[str, tuple[str, str]] = {}

    @classmethod
    def read(cls, value: object, label: str) -> "SourceEvidence":
        """Read a retained inventory and its use rows."""
        inventory = read_object(value, f"{label} inventory")
        evidence = cls(label)
        for item in read_array(inventory.get("sources"), f"{label} sources"):
            evidence.source(read_object(item, f"{label} source"))
        for item in read_array(inventory.get("uses"), f"{label} uses"):
            evidence.source_use(read_object(item, f"{label} use"))
        evidence.check_ancestors()
        return evidence

    def source(self, source: dict[str, Any]) -> None:
        identity = source.get("id")
        ancestors = source.get("ancestors")
        if (
            not _identity(identity) or identity in self.sources
            or not isinstance(ancestors, list) or any(not _identity(value) for value in ancestors)
            or len(set(ancestors)) != len(ancestors) or identity in ancestors
            or source.get("storedEncryption") not in _ENCRYPTION
        ):
            raise PackageError(f"A {self.label} physical source is malformed or duplicated.")
        self.sources[identity] = tuple(ancestors)

    def source_use(self, use: dict[str, Any]) -> None:
        identity = use.get("id")
        source = use.get("source")
        scope = use.get("scope")
        role = use.get("role")
        encryption = use.get("effectiveEncryption")
        if (
            not _identity(identity) or identity in self.uses or not _identity(source) or source not in self.sources
            or scope not in _SCOPES or role not in _ROLES or encryption not in _ENCRYPTION
            or not isinstance(use.get("location"), dict)
        ):
            raise PackageError(f"A {self.label} source use is malformed or duplicated.")
        self.uses[identity] = role, encryption

    def check_origin(self, value: object) -> None:
        """Check that every declared contributor and its encryption marker agree."""
        origin = read_object(value, f"{self.label} field origin")
        if origin.keys() != {"containsEncryptedSource", "sourceSetComplete", "sourceUsesRefs"}:
            raise PackageError(f"A {self.label} field origin is malformed.")
        references = origin["sourceUsesRefs"]
        if (
            not isinstance(references, list)
            or any(not _identity(ref) or ref not in self.uses for ref in references)
            or len(set(references)) != len(references)
            or any(self.uses[ref][0] not in _CONTRIBUTORS for ref in references)
        ):
            raise PackageError(f"A {self.label} field has unresolved contributor references.")
        complete = origin["sourceSetComplete"]
        if type(complete) is not bool:
            raise PackageError(f"A {self.label} field has an invalid completeness marker.")
        encryption = {self.uses[ref][1] for ref in references}
        marker = True if "encrypted" in encryption else False if complete and "unknown" not in encryption else None
        if origin["containsEncryptedSource"] is not marker:
            raise PackageError(f"A {self.label} field encryption marker differs from its sources.")

    def check_ancestors(self) -> None:
        """Check closure of physical containing Entry references."""
        if any(ancestor not in self.sources for ancestors in self.sources.values() for ancestor in ancestors):
            raise PackageError(f"The {self.label} source identities are incomplete.")


def _identity(value: Any) -> bool:
    return isinstance(value, str) and bool(value)


def follows_pointer(value: Any, pointer: str) -> bool:
    """Checks whether an exact escaped JSON Pointer resolves in the supplied value."""
    if not pointer.startswith("/"):
        return pointer == ""
    for part in pointer.split("/")[1:]:
        if "~" in part.replace("~0", "").replace("~1", ""):
            return False
        key = part.replace("~1", "/").replace("~0", "~")
        if isinstance(value, dict) and key in value:
            value = value[key]
        elif (
            isinstance(value, list) and key.isascii() and key.isdecimal()
            and (key == "0" or not key.startswith("0"))
            and len(key) <= len(str(len(value))) and int(key) < len(value)
        ):
            value = value[int(key)]
        else:
            return False
    return True

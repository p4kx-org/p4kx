"""Stops only captured portal identities through Linux pidfds on disposable runners."""

import json
import os
from pathlib import Path
import select
import signal
import sys


def validate(record):
    """Requires one captured positive PID, kernel start identity and absolute executable."""
    if (
        type(record.get("pid")) is not int
        or record["pid"] <= 0
        or not isinstance(record.get("start"), str)
        or not record["start"].isdigit()
        or not isinstance(record.get("executable"), str)
        or not record["executable"].startswith("/")
        or record.get("service") not in {
            "org.freedesktop.portal.Documents",
            "org.freedesktop.impl.portal.PermissionStore",
        }
    ):
        raise ValueError("invalid owned portal identity")


class Native:
    """Provides exact pidfd signaling and kernel identity reads."""

    def open(self, pid):
        return os.pidfd_open(pid)

    def identity(self, pid):
        text = Path(f"/proc/{pid}/stat").read_text(encoding="utf-8")
        fields = text.rsplit(") ", 1)[1].split()
        return fields[19], str(Path(f"/proc/{pid}/exe").resolve(strict=True))

    def wait(self, fd, milliseconds):
        poller = select.poll()
        poller.register(fd, select.POLLIN)
        return bool(poller.poll(milliseconds))

    def send(self, fd, signum):
        signal.pidfd_send_signal(fd, signum)

    def close(self, fd):
        os.close(fd)


def stop_opened(record, native, fd):
    """Signals an open pidfd only after matching its captured kernel identity."""
    if native.wait(fd, 0):
        return
    try:
        actual = native.identity(record["pid"])
    except FileNotFoundError:
        if native.wait(fd, 0):
            return
        raise
    if actual != (record["start"], record["executable"]):
        raise ValueError("owned portal process identity changed; no signal sent")
    try:
        native.send(fd, signal.SIGTERM)
        if not native.wait(fd, 5000):
            native.send(fd, signal.SIGKILL)
            if not native.wait(fd, 1000):
                raise TimeoutError("owned portal did not exit after bounded cleanup")
    except ProcessLookupError:
        if not native.wait(fd, 0):
            raise


def stop_owned(record, native):
    """Retains original and close errors while binding all signals to an owned pidfd."""
    validate(record)
    try:
        fd = native.open(record["pid"])
    except ProcessLookupError:
        return
    try:
        stop_opened(record, native, fd)
    except Exception as original:
        try:
            native.close(fd)
        except Exception as cleanup:
            raise ExceptionGroup("owned portal operation and close failed", [original, cleanup]) from None
        raise
    else:
        native.close(fd)


def main():
    """Reads only the owned session proof and performs guarded cleanup."""
    if (
        sys.platform != "linux"
        or os.environ.get("GITHUB_ACTIONS") != "true"
        or os.environ.get("RUNNER_ENVIRONMENT") != "github-hosted"
        or len(sys.argv) != 2
    ):
        raise RuntimeError("owned portal cleanup requires a disposable native runner")
    proof = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8-sig"))
    if proof.get("inventory_complete") is not True or len(proof["services"]) > 4:
        raise ValueError("owned portal inventory is incomplete")
    failures = []
    for record in proof["services"]:
        try:
            stop_owned(record, Native())
        except Exception as error:
            failures.append(error)
    if failures:
        raise ExceptionGroup("owned portal cleanup failed", failures)


if __name__ == "__main__":
    main()

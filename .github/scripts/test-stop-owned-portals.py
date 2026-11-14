"""Checks pidfd cleanup with injected operations and no native process access."""

import importlib.util
from pathlib import Path
import signal
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location("owned_portal", Path(__file__).with_name("stop-owned-portals.py"))
PORTAL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PORTAL)
RECORD = {"pid": 123, "start": "456", "executable": "/owned/xdg-document-portal", "service": "org.freedesktop.portal.Documents"}


class Fake:
    """Records controller decisions without opening files, handles or processes."""

    def __init__(self, waits, identity=("456", "/owned/xdg-document-portal"), close_error=None):
        self.waits = iter(waits)
        self.actual = identity
        self.close_error = close_error
        self.sent = []
        self.closed = []

    def open(self, pid):
        if pid != RECORD["pid"]:
            raise ValueError("unexpected synthetic PID")
        return 9

    def identity(self, pid):
        return self.actual

    def wait(self, fd, milliseconds):
        return next(self.waits)

    def send(self, fd, signum):
        self.sent.append((fd, signum))

    def close(self, fd):
        self.closed.append(fd)
        if self.close_error:
            raise self.close_error


class CleanupTests(unittest.TestCase):
    def test_matching_identity_gets_term_and_exact_close(self):
        native = Fake([False, True])
        PORTAL.stop_owned(RECORD, native)
        self.assertEqual(native.sent, [(9, signal.SIGTERM)])
        self.assertEqual(native.closed, [9])

    def test_already_exited_handle_gets_no_signal(self):
        native = Fake([True])
        PORTAL.stop_owned(RECORD, native)
        self.assertEqual(native.sent, [])
        self.assertEqual(native.closed, [9])

    def test_reused_pid_or_changed_executable_gets_no_signal(self):
        for identity in [("999", RECORD["executable"]), ("456", "/foreign/program")]:
            native = Fake([False], identity)
            with self.assertRaises(ValueError):
                PORTAL.stop_owned(RECORD, native)
            self.assertEqual(native.sent, [])
            self.assertEqual(native.closed, [9])

    def test_hung_owned_process_uses_bounded_escalation(self):
        native = Fake([False, False, True])
        with mock.patch.object(signal, "SIGKILL", 9, create=True):
            PORTAL.stop_owned(RECORD, native)
        self.assertEqual(native.sent, [(9, signal.SIGTERM), (9, 9)])
        self.assertEqual(native.closed, [9])

    def test_unresolved_hang_stays_failure(self):
        native = Fake([False, False, False])
        with mock.patch.object(signal, "SIGKILL", 9, create=True), self.assertRaises(TimeoutError):
            PORTAL.stop_owned(RECORD, native)
        self.assertEqual(native.closed, [9])

    def test_original_and_cleanup_error_remain_inspectable(self):
        cleanup = OSError("synthetic close")
        native = Fake([False], ("different", RECORD["executable"]), cleanup)
        with self.assertRaises(ExceptionGroup) as raised:
            PORTAL.stop_owned(RECORD, native)
        self.assertIsInstance(raised.exception.exceptions[0], ValueError)
        self.assertIs(raised.exception.exceptions[1], cleanup)

    def test_foreign_or_malformed_record_is_rejected_before_open(self):
        for patch in [{"pid": 0}, {"pid": True}, {"start": ""}, {"executable": "relative"}, {"service": "foreign"}]:
            with self.assertRaises(ValueError):
                PORTAL.stop_owned(RECORD | patch, Fake([]))


if __name__ == "__main__":
    unittest.main()

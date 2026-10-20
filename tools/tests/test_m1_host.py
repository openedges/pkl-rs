"""Host observation checks must detect absent callbacks and invalid raw results."""
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m1_host


class HostObservationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.events = {
            "warmup": "ok", "first": "ok", "repeat": "ok", "second-module": "ok",
            "reader-calls": "1", "fresh": "ok", "reader-calls-after-fresh": "2",
            "close-once": "ok", "close-twice": "ok",
            "after-final-close": "org.graalvm.polyglot.PolyglotException"}
        self.values = {"warmup": b"1", "first": b"payload-1", "repeat": b"payload-1",
                       "second-module": b"payload-1", "fresh": b"payload-2",
                       "close-once": b"closed", "close-twice": b"closed"}
        for name, value in self.values.items():
            (self.root / f"{name}.result.bin").write_bytes(value)
        (self.root / "after-final-close.error.bin").write_bytes(b"Context execution was cancelled.")
        self.save_events()

    def save_events(self):
        (self.root / "stdout.bin").write_bytes("".join(f"{key}\t{value}\n" for key, value in self.events.items()).encode())

    def test_complete_cached_and_fresh_observations(self):
        _, checks = m1_host.observe("repeat", self.root)
        self.assertTrue(all(checks.values()))

    def test_whitespace_byte_difference_is_not_trimmed(self):
        (self.root / "first.result.bin").write_bytes(b"payload-1\n")
        _, checks = m1_host.observe("repeat", self.root)
        self.assertFalse(checks["same-module-cached"])

    def test_absent_reader_call_or_wrong_cache_count_fails(self):
        for value in ["0", "2"]:
            self.events["reader-calls"] = value
            self.save_events()
            _, checks = m1_host.observe("repeat", self.root)
            self.assertFalse(checks["resource-cache-across-modules"])

    def test_unexpected_afterclose_error_is_not_a_pass(self):
        self.events["after-final-close"] = "org.pkl.core.PklException"
        self.save_events()
        _, checks = m1_host.observe("repeat", self.root)
        self.assertFalse(checks["after-close-rejected"])

    def test_duplicate_malformed_and_unknown_events_fail(self):
        for data in [b"name\tone\nname\ttwo\n", b"no-tab\n"]:
            with self.assertRaises(ValueError):
                m1_host.events(data)
        with self.assertRaises(ValueError):
            m1_host.observe("unknown", self.root)

    def test_missing_scalar_capture_is_an_acquisition_error(self):
        (self.root / "first.result.bin").unlink()
        with self.assertRaises(OSError):
            m1_host.observe("repeat", self.root)

    def test_empty_observations_cannot_pass(self):
        (self.root / "stdout.bin").write_bytes(b"")
        _, checks = m1_host.observe("repeat", self.root)
        self.assertFalse(all(checks.values()))

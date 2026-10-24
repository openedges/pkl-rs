"""Raw collection conversion comparator and build provenance regressions."""
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import collection_conversions_compare as compare


class CollectionConversionsCompareTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.output = Path(self.temp.name) / "capture"
        self.output.mkdir()
        (self.output / "fixture").mkdir()
        (self.output / "build").mkdir()
        (self.output / "build/string_session_probe").write_bytes(b"native artifact")
        self.uri = "file:///stage62/synthetic.pkl"
        self.cases = {"module_uri": self.uri,
                      "rows": [{"id": "value", "expression": "1.ifNonNull((x)->x)"},
                               {"id": "error", "expression": "1.ifNonNull((x)->bad())"}]}
        compare.put(self.output / "fixture/cases.json", self.cases)
        compare.put(self.output / "report.json", {"output": str(self.output)})
        self.java = {"provenance.json": json.dumps({"environment": {"LC_ALL": "C", "TZ": "UTC"}}).encode()}
        self.processes = []
        for repeat in range(2):
            folder = self.output / "native" / f"repeat-{repeat}"
            folder.mkdir(parents=True)
            protocol = ["session-open mode=string rows=2"]
            for index, row in enumerate(self.cases["rows"]):
                status = b"success\n" if index == 0 else b"error\n"
                stdout = b"1" if index == 0 else b""
                stderr = b"" if index == 0 else b"complete Java error\r\n"
                values = {"expression.bin": row["expression"].encode(),
                          "expression.code-units.bin": row["expression"].encode("utf-16-be"),
                          "result.status.bin": status, "result.stdout.bin": stdout,
                          "result.stdout.code-units.bin": stdout.decode().encode("utf-16-be"),
                          "result.stderr.bin": stderr,
                          "result.stderr.code-units.bin": stderr.decode().encode("utf-16-be")}
                name = f"{index:03d}-{row['id']}"
                candidate = folder / name
                candidate.mkdir()
                for channel, raw in values.items():
                    self.java[f"repeat-{repeat}/{name}/{channel}"] = raw
                    (candidate / channel).write_bytes(raw)
                for channel in compare.EVENT_METADATA:
                    (candidate / channel).write_bytes(b"language-error" if channel == "candidate.disposition.bin"
                                                       and index == 1 else b"")
                call = f"string-{index}-{row['id']}"
                protocol += [f"call-enter={call}", f"call-exit={call} status={status.decode().strip()}"]
            protocol += ["session-closed mode=string rows=2 public-calls=2 success=1 errors=1"]
            process_stdout = ("\n".join(protocol) + "\n").encode()
            for channel, raw in (("stdout", process_stdout), ("stderr", b"")):
                (folder / f"process.{channel}.bin").write_bytes(raw)
                self.java[f"repeat-{repeat}/process.{channel}.bin"] = raw
            (folder / "candidate-trace.tsv").write_bytes(
                b"terminal-heap-slots\t0\nterminal-context-depth\t0\n")
            self.processes.append({"repeat": repeat, "pid": 100 + repeat, "started_epoch": 10 + repeat * 2,
                                   "finished_epoch": 11 + repeat * 2, "exit_code": 0,
                                   "timed_out": False, "launch_error": None, "timeout_seconds": 120,
                                   "environment": {"LC_ALL": "C", "TZ": "UTC"},
                                   "command": [str(self.output / "build/string_session_probe"), str(folder),
                                               str(self.output / "fixture"), self.uri],
                                   "cwd": str(self.output / "fixture"),
                                   "stdout": compare.identity(folder / "process.stdout.bin"),
                                   "stderr": compare.identity(folder / "process.stderr.bin")})

    def verdict(self):
        profile = {'module_uri': self.uri, 'rows': 2, 'repetitions': 2}
        with mock.patch.object(compare, "java_files", return_value=self.java), \
             mock.patch.object(compare.profiles, "profile_for_fixture", return_value=profile):
            return compare.compare(self.output, self.processes)

    def test_complete_synthetic_producer_capture_passes(self):
        verdict = self.verdict()
        self.assertEqual(verdict["counts"], {"pass": 4})
        self.assertTrue(verdict["strict_string_passed"])

    def test_value_error_crlf_and_utf16_changes_are_raw_mismatches(self):
        changes = [("000-value/result.stdout.bin", b"1\r\n", "result.stdout.bin"),
                   ("001-error/result.stderr.bin", b"complete Java error\n", "result.stderr.bin"),
                   ("000-value/result.stdout.code-units.bin", b"\x001\x00\r", "result.stdout.code-units.bin")]
        for relative, replacement, expected_channel in changes:
            with self.subTest(relative=relative):
                path = self.output / "native/repeat-0" / relative
                before = path.read_bytes()
                path.write_bytes(replacement)
                verdict = self.verdict()
                self.assertFalse(verdict["strict_string_passed"])
                changed = [row for row in verdict["rows"] if row["status"] == "mismatch"]
                self.assertEqual([(row["repeat"], row["different_channels"]) for row in changed],
                                 [(0, [expected_channel])])
                path.write_bytes(before)

    def test_status_difference_cannot_be_hidden_by_matching_values(self):
        row = self.output / "native/repeat-0/001-error"
        (row / "result.status.bin").write_bytes(b"candidate-error\n")
        (row / "candidate.disposition.bin").write_bytes(b"unsupported")
        process_path = self.output / "native/repeat-0/process.stdout.bin"
        changed = process_path.read_bytes().replace(b"status=error\n", b"status=candidate-error\n")
        process_path.write_bytes(changed)
        self.processes[0]["stdout"] = compare.identity(process_path)
        verdict = self.verdict()
        self.assertEqual([row["different_channels"] for row in verdict["rows"] if row["status"] == "mismatch"],
                         [["result.status.bin"]])
        self.assertEqual(verdict["sessions"][0]["different_process_channels"], ["stdout"])

    def test_missing_or_extra_raw_channel_is_rejected(self):
        row = self.output / "native/repeat-0/000-value"
        missing = row / "result.stderr.code-units.bin"
        missing.unlink()
        with self.assertRaisesRegex(ValueError, "channel set differs"):
            self.verdict()
        missing.write_bytes(b"")
        (row / "extra.bin").write_bytes(b"")
        with self.assertRaisesRegex(ValueError, "channel set differs"):
            self.verdict()

    def test_missing_duplicate_or_overlapping_sessions_are_rejected(self):
        for processes in (self.processes[:1], [self.processes[0], self.processes[0]],
                          [self.processes[0], {**self.processes[1], "pid": 100}],
                          [self.processes[0], {**self.processes[1], "started_epoch": 10.5}]):
            with self.subTest(processes=processes), self.assertRaises(ValueError):
                with mock.patch.object(compare, "java_files", return_value=self.java), \
                     mock.patch.object(compare.profiles, "profile_for_fixture", return_value={
                         'module_uri': self.uri, 'rows': 2, 'repetitions': 2}):
                    compare.compare(self.output, processes)

    def test_process_environment_and_timeout_are_bound_to_oracle(self):
        bad_environment = [{**self.processes[0], "environment": {"LC_ALL": "C", "TZ": "Asia/Seoul"}},
                           self.processes[1]]
        with mock.patch.object(compare, "java_files", return_value=self.java), \
             mock.patch.object(compare.profiles, "profile_for_fixture", return_value={
                 'module_uri': self.uri, 'rows': 2, 'repetitions': 2}):
            with self.assertRaisesRegex(ValueError, "environment"):
                compare.compare(self.output, bad_environment)
        bad_timeout = [{**self.processes[0], "timeout_seconds": 5}, self.processes[1]]
        with mock.patch.object(compare, "java_files", return_value=self.java), \
             mock.patch.object(compare.profiles, "profile_for_fixture", return_value={
                 'module_uri': self.uri, 'rows': 2, 'repetitions': 2}):
            with self.assertRaisesRegex(ValueError, "timeout"):
                compare.compare(self.output, bad_timeout)

    def test_ten_sealed_profiles_and_wrong_count_uri_or_input_are_rejected(self):
        expected = {
            'file:///stage62/collection-conversions.pkl': 236,
            'file:///stage62/collection-conversion-controls.pkl': 58,
            'file:///stage62/collection-lookups.pkl': 133,
            'file:///stage62/collection-default-contracts.pkl': 22,
            'file:///stage62/dynamic-render.pkl': 10,
            'file:///stage62/collection-property-names.pkl': 26,
            'file:///stage63/collection-addition.pkl': 125,
            'file:///stage63/set-endpoints.pkl': 70,
            'file:///stage64/amendment-construction.pkl': 188,
            'file:///stage65/inferred-new-values.pkl': 72,
        }
        self.assertEqual({uri: profile['rows'] for uri, profile in compare.profiles.PROFILES.items()},
                         expected)
        self.assertEqual(sum(expected.values()), 940)
        for uri, rows in expected.items():
            profile = compare.profiles.PROFILES[uri]
            fixture = compare.ROOT / profile['fixture']
            self.assertEqual(compare.profiles.profile_for_fixture(fixture)['rows'], rows)
            self.assertEqual(compare.profiles.authority_for_fixture(fixture).verify(fixture)['rows'], rows)
            copied = self.output / ('profile-' + str(rows))
            copied.mkdir()
            for name in compare.profiles.INPUTS:
                shutil.copy2(fixture / name, copied / name)
            self.assertEqual(compare.profiles.profile_for_fixture(copied)['module_uri'], uri)
            cases = json.loads((copied / 'cases.json').read_bytes())
            for change in (
                lambda value: value['rows'].pop(),
                lambda value: value.update(module_uri='file:///stage62/unknown.pkl'),
                lambda value: value.update(module_uri='file:///stage63/unknown.pkl'),
                lambda value: value.update(module_uri='file:///stage64/unknown.pkl'),
                lambda value: value.update(repetitions=1),
            ):
                modified = json.loads(json.dumps(cases))
                change(modified)
                (copied / 'cases.json').write_text(json.dumps(modified))
                with self.assertRaises(ValueError):
                    compare.profiles.profile_for_fixture(copied)
            (copied / 'cases.json').write_bytes((fixture / 'cases.json').read_bytes())
            (copied / 'module.pkl').write_bytes(b'changed source')
            with self.assertRaisesRegex(ValueError, 'input identity'):
                compare.profiles.profile_for_fixture(copied)

    def test_profile_catalog_rejects_duplicate_or_unallowlisted_dispatch(self):
        catalog = json.loads((compare.ROOT / compare.profiles.CATALOG).read_bytes())
        path = self.output / 'catalog.json'
        for change in (
            lambda value: value['profiles'].append(dict(value['profiles'][0])),
            lambda value: value['profiles'][0].update(validator_module='untrusted_fixtures'),
            lambda value: value['profiles'][0].update(rows=0),
            lambda value: value['profiles'][0]['tool_sources'].remove(
                'tools/collection_conversions_oracle.py'),
            lambda value: value['profiles'][0]['input_hashes'].update({'cases.json': '0' * 64}),
        ):
            modified = json.loads(json.dumps(catalog))
            change(modified)
            path.write_text(json.dumps(modified))
            if modified['profiles'][0]['input_hashes']['cases.json'] == '0' * 64:
                # A valid-looking hash is admitted by the catalog parser, then
                # rejected against the fixture by profile_for_fixture.
                with mock.patch.object(compare.profiles, 'PROFILES',
                                       compare.profiles.load_profiles(path)):
                    fixture = compare.ROOT / modified['profiles'][0]['fixture']
                    with self.assertRaisesRegex(ValueError, 'input identity'):
                        compare.profiles.profile_for_fixture(fixture)
            else:
                with self.assertRaises(ValueError):
                    compare.profiles.load_profiles(path)

    def test_lookup_profile_can_enter_without_new_comparator_dispatch(self):
        uri = 'file:///stage62/collection-lookups.pkl'
        fixture = compare.ROOT / 'tests/fixtures/m2-collection-lookups'
        catalog = json.loads((compare.ROOT / compare.profiles.CATALOG).read_bytes())
        if uri not in {profile['module_uri'] for profile in catalog['profiles']}:
            catalog['profiles'].append({
                'module_uri': uri, 'fixture': fixture.relative_to(compare.ROOT).as_posix(),
                'rows': 133, 'repetitions': 2,
                'validator_module': 'collection_lookups_fixtures',
                'validator_script': 'tools/collection_lookups_fixtures.py',
                'archive_prefix': 'capture-03',
                'input_hashes': {name: compare.profiles.digest(fixture / name)
                                 for name in compare.profiles.INPUTS},
                'tool_sources': sorted(compare.profiles.PROFILE_TOOL_CLOSURES[
                    'collection_lookups_fixtures']),
            })
        path = self.output / 'lookup-catalog.json'
        path.write_text(json.dumps(catalog))
        with mock.patch.object(compare.profiles, 'PROFILES',
                               compare.profiles.load_profiles(path)):
            self.assertEqual(compare.profiles.profile_for_fixture(fixture)['rows'], 133)
            self.assertEqual(compare.profiles.authority_for_fixture(fixture).verify(fixture)
                             ['statuses'], {'success': 82, 'error': 51})
            java = compare.java_files(fixture)
            self.assertEqual(java['repeat-0/000-dyn-has-empty/result.stdout.bin'], b'false')

    def test_new_profile_authorities_and_transitive_sources(self):
        for uri, rows, statuses, first in (
            ('file:///stage62/collection-default-contracts.pkl', 22,
             {'success': 12, 'error': 10},
             'repeat-0/000-list-nonfunction-missing/result.status.bin'),
            ('file:///stage62/dynamic-render.pkl', 10,
             {'success': 10},
             'repeat-0/000-high-to-string/result.stdout.code-units.bin'),
            ('file:///stage62/collection-property-names.pkl', 26,
             {'success': 26},
             'repeat-0/000-authored-direct/result.stdout.code-units.bin'),
            ('file:///stage63/collection-addition.pkl', 125,
             {'success': 86, 'error': 39},
             'repeat-0/000-ll-ordered/result.stdout.code-units.bin'),
            ('file:///stage63/set-endpoints.pkl', 70,
             {'success': 47, 'error': 23},
             'repeat-0/000-set-empty-first/result.stderr.code-units.bin'),
            ('file:///stage64/amendment-construction.pkl', 188,
             {'success': 102, 'error': 86},
             'repeat-0/000-module-first/result.stdout.code-units.bin'),
            ('file:///stage65/inferred-new-values.pkl', 72,
             {'success': 49, 'error': 23},
             'repeat-0/000-module-self-type/result.stdout.code-units.bin'),
        ):
            with self.subTest(uri=uri):
                profile = compare.profiles.PROFILES[uri]
                fixture = compare.ROOT / profile['fixture']
                self.assertEqual(compare.profiles.authority_for_fixture(fixture).verify(fixture)
                                 ['statuses'], statuses)
                self.assertIn(first, compare.java_files(fixture))
                self.assertEqual(profile['rows'], rows)
                self.assertTrue(set(profile['tool_sources']) <= set(compare.TOOLS))
                self.assertEqual(set(profile['tool_sources']),
                                 compare.profiles.PROFILE_TOOL_CLOSURES[profile['validator_module']])

    def test_inferred_new_values_verifier_requires_explicit_profile(self):
        uri = 'file:///stage65/inferred-new-values.pkl'
        profile = compare.profiles.PROFILES[uri]
        fixture = compare.ROOT / profile['fixture']
        self.assertEqual(compare.profiles.validator_args(profile, fixture), [
            str(compare.ROOT / 'tools/inferred_new_fixtures.py'),
            '--profile', 'values', '--fixture', str(fixture)])
        old = compare.profiles.PROFILES['file:///stage64/amendment-construction.pkl']
        old_fixture = compare.ROOT / old['fixture']
        self.assertEqual(compare.profiles.validator_args(old, old_fixture), [
            str(compare.ROOT / old['validator_script']), '--fixture', str(old_fixture)])

    def test_relocated_capture_uses_recorded_path_for_command_identity(self):
        recorded = self.output / "original-place"
        compare.put(self.output / "report.json", {"output": str(recorded)})
        for repeat, process in enumerate(self.processes):
            process["command"] = [str(recorded / "build/string_session_probe"),
                                  str(recorded / "native" / f"repeat-{repeat}"),
                                  str(recorded / "fixture"), self.uri]
            process["cwd"] = str(recorded / "fixture")
        self.assertTrue(self.verdict()["strict_string_passed"])

    def test_build_source_and_artifact_bytes_are_verified(self):
        build = self.output / "synthetic-build"
        sources = build / "sources"
        sources.mkdir(parents=True)
        (sources / "input.rs").write_bytes(b"source A")
        before = {"input.rs": compare.identity(sources / "input.rs")}
        compare.put(build / "source-before.json", before)
        compare.put(build / "source-after.json", before)
        for name in ("string_session_probe", "data_size_fixture_probe"):
            (build / name).write_bytes(("artifact " + name).encode())
        for channel in ("stdout", "stderr"):
            (build / f"build.{channel}.bin").write_bytes(b"build " + channel.encode())
        report = {"source_stable": True, "complete": True, "exit_code": 0,
                  "argv": ["cargo", "build", "-p", "pkl-runtime", "--example", "string_session_probe",
                           "--example", "data_size_fixture_probe", "--locked"],
                  "env": {"CARGO_INCREMENTAL": "0", "CARGO_PROFILE_DEV_DEBUG": "0"},
                  "artifacts": {name: compare.identity(build / name)
                                for name in ("string_session_probe", "data_size_fixture_probe")},
                  **{channel: compare.identity(build / f"build.{channel}.bin")
                     for channel in ("stdout", "stderr")}}
        compare.put(build / "build.json", report)
        with mock.patch.object(compare, "build_inputs", return_value={"input.rs"}):
            self.assertEqual(compare.verify_build(build), report["artifacts"])
            (sources / "input.rs").write_bytes(b"source B")
            with self.assertRaisesRegex(ValueError, "captured build source differs"):
                compare.verify_build(build)
            (sources / "input.rs").write_bytes(b"source A")
            (build / "string_session_probe").write_bytes(b"different artifact")
            with self.assertRaisesRegex(ValueError, "built artifact differs"):
                compare.verify_build(build)

    def test_embedded_stdlib_is_mandatory_even_when_snapshot_omits_it(self):
        build = self.output / "incomplete-build"
        sources = build / "sources"
        sources.mkdir(parents=True)
        minimum = {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
                   "crates/pkl-runtime/Cargo.toml",
                   "crates/pkl-runtime/examples/string_session_probe.rs",
                   "crates/pkl-runtime/examples/data_size_fixture_probe.rs",
                   "crates/pkl-runtime/examples/support/string_session.rs"}
        for name in minimum | compare.EMBEDDED_INPUTS:
            path = sources / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"recorded source")
        before = {name: compare.identity(sources / name) for name in minimum | compare.EMBEDDED_INPUTS}
        self.assertTrue(compare.build_inputs(sources).issubset(before))
        victim = "crates/pkl-runtime/src/stdlib/ref.pkl"
        (sources / victim).unlink()
        del before[victim]
        compare.put(build / "source-before.json", before)
        compare.put(build / "source-after.json", before)
        compare.put(build / "build.json", {"source_stable": True})
        with self.assertRaisesRegex(ValueError, "build source closure incomplete"):
            compare.verify_build(build)


if __name__ == "__main__":
    unittest.main()

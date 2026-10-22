import struct
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m2_uppercase_tables as tables


def inventory(count, replacements=None):
    replacements = replacements or {}
    payload = bytearray(b'PKLUPR1\0' + struct.pack('>I', count))
    for point in range(count):
        units = replacements.get(point, tables.original_units(point))
        payload.extend(struct.pack('>H', len(units)))
        for unit in units:
            payload.extend(struct.pack('>H', unit))
    return bytes(payload)


class UppercaseTransportTests(unittest.TestCase):
    def test_complete_inventory_retains_expansions(self):
        with patch.object(tables, 'TOTAL', 0xe0):
            payload = inventory(0xe0, {0x61: (0x41,), 0xdf: (0x53, 0x53)})
            self.assertEqual(tables.mappings(payload), [(0x61, (0x41,)), (0xdf, (0x53, 0x53))])

    def test_missing_rows_wrong_count_and_trailing_data_fail(self):
        with patch.object(tables, 'TOTAL', 4):
            payload = inventory(4)
            for invalid in [b'', payload[:8], inventory(3), payload[:-1], payload + b'\0']:
                with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                    tables.mappings(invalid)

    def test_empty_or_excessive_mapping_fails(self):
        with patch.object(tables, 'TOTAL', 1):
            for units in [(), (1, 2, 3, 4)]:
                with self.subTest(units=units), self.assertRaises(ValueError):
                    tables.mappings(inventory(1, {0: units}))

    def test_surrogate_replacement_cannot_be_published_as_identity(self):
        with patch.object(tables, 'TOTAL', 0xd801):
            payload = inventory(0xd801)
            self.assertEqual(tables.mappings(payload), [])
            with self.assertRaisesRegex(ValueError, 'unmatched surrogate'):
                tables.mappings(payload[:-2] + b'\0?')

    def test_source_drift_preserves_target_and_records_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tool = root / 'tools/m2_uppercase_tables.py'
            sources = [tables.PROBE, tool.relative_to(root), Path('tools/oracle.py'),
                       Path('oracle.lock.json'), Path('references.lock'),
                       Path('external/pkl/pkl-core/src/main/java/org/pkl/core/stdlib/base/StringNodes.java')]
            for name in sources:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b'initial source\n')
            target = root / tables.TARGET
            target.parent.mkdir(parents=True)
            target.write_bytes(b'existing table\n')
            metadata = dict(command=['java', 'cli'], environment={}, jdk=dict(properties={
                'java.vendor': 'test', 'java.runtime.version': 'test'}))
            original_render = tables.render

            def changed_source(*args):
                generated = original_render(*args)
                (root / 'references.lock').write_bytes(b'changed source\n')
                return generated

            with (patch.object(tables, 'ROOT', root), patch.object(tables, '__file__', str(tool)),
                  patch.object(tables, 'TOTAL', 4),
                  patch.object(tables.oracle, 'ensure_oracle', return_value=metadata),
                  patch.object(tables.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, inventory(4), b'')),
                  patch.object(tables, 'render', side_effect=changed_source)):
                with self.assertRaisesRegex(ValueError, 'inputs changed'):
                    tables.execute(root / 'evidence', True)
            self.assertEqual(target.read_bytes(), b'existing table\n')
            report = json.loads((root / 'evidence/report.json').read_bytes())
            self.assertEqual(report['status'], 'failed')
            self.assertFalse(report['source_stability'])


if __name__ == '__main__':
    unittest.main()

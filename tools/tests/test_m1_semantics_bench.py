"""Timing evidence must retain unstable, incomplete and incorrect outcomes."""
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m1_semantics_bench as bench


class NumericCostEvidenceTests(unittest.TestCase):
    def test_drift_is_inconclusive(self):
        self.assertEqual(bench.stability([100]*6+[160]*6)['status'], 'inconclusive')

    def test_stable_measurements(self):
        self.assertEqual(bench.stability([100]*12)['status'], 'stable')

    def test_missing_or_zero_measurements_rejected(self):
        for samples in ([], [100]*11, [0]*12):
            with self.assertRaises(ValueError): bench.stability(samples)

    def test_raw_correctness_mismatch_rejected(self):
        with self.assertRaises(ValueError): bench.parse_samples(b'changed\n', b'expected\n')

    def test_missing_samples_rejected(self):
        with self.assertRaises(ValueError): bench.parse_samples(b'expected\nsample\n', b'expected\n')

    def test_complete_samples_preserve_phases(self):
        prefix=b'expected\n'
        rows=[]
        for i in range(12):
            for op in bench.OPERATIONS:
                rows.append(f'sample\t{op}\twarmup\t{i}\t100\t0000000000000000\n'.encode())
        rows.append(b'warmup-status\tstable\t5000000000\t12\n')
        for op in bench.OPERATIONS:
            for i in range(12):
                rows.append(f'sample\t{op}\tmeasurement\t{i}\t100\t0000000000000000\n'.encode())
        parsed,warmup=bench.parse_samples(prefix+b''.join(rows),prefix)
        self.assertEqual(parsed['sin'][0]['phase'],'warmup')
        self.assertEqual(parsed['sin'][12]['phase'],'measurement')
        self.assertEqual(warmup['status'],'stable')
        with self.assertRaises(ValueError): bench.parse_samples(prefix+b''.join(reversed(rows)),prefix)
        with self.assertRaises(ValueError): bench.parse_samples((prefix+b''.join(rows)).replace(b'5000000000',b'1'),prefix)


if __name__ == '__main__': unittest.main()

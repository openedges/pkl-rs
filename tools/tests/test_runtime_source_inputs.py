"""Compiled embedded inputs must participate in evidence source identities."""
from pathlib import Path
import re
import sys
import unittest

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
import m1_platforms
import m2_source_inputs


class EmbeddedRuntimeInputTests(unittest.TestCase):
    def test_all_literal_runtime_embeds_are_captured(self):
        root = TOOLS.parent
        sources = {path: path.read_text()
                   for path in (root / 'crates/pkl-runtime/src').rglob('*.rs')}
        test_modules = set()
        for source, text in sources.items():
            for name in re.findall(r'#\[cfg\(test\)\]\s*mod\s+(\w+)\s*;', text):
                directory = source.parent if source.name in ('lib.rs', 'main.rs', 'mod.rs') else source.with_suffix('')
                test_modules.add(directory / (name + '.rs'))
        embedded = set()
        for source, text in sources.items():
            # Externally declared cfg(test) modules are not compiled into the
            # native/Node/browser evaluator artifacts captured by these tools.
            if source in test_modules:
                continue
            for relative in re.findall(r'include_(?:str|bytes)!\s*\(\s*"([^"]+)"', text):
                embedded.add((source.parent / relative).resolve())
        self.assertTrue(embedded)
        self.assertTrue(all(path.is_file() for path in embedded))
        catalogs = {
            'm1-platforms': set(m1_platforms.source_paths(root)),
            'm2-shared': {root / name for name in m2_source_inputs.source_paths(root)},
        }
        for name, captured in catalogs.items():
            with self.subTest(catalog=name):
                self.assertEqual(embedded - captured, set())


if __name__ == '__main__':
    unittest.main()

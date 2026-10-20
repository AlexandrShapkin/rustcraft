"""Regression tests for missing sections, common drift and lost native gates."""
from pathlib import Path
import tempfile
import unittest

import check_agent_contract as contract

ROOT = Path(__file__).resolve().parents[1]


class AgentContractTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / '.github').mkdir()
        for name in ['AGENTS.md', '.github/DEVELOPMENT_INTERFACE.md']:
            (self.root / name).write_bytes((ROOT / name).read_bytes())
        text = (self.root / 'AGENTS.md').read_text(encoding='utf-8')
        self.project = next(p for p in contract.REQUIRED if '<!-- agent-project: ' + p + ' -->' in text)

    def mutate(self, name, before, after):
        path = self.root / name
        path.write_text(path.read_text(encoding='utf-8').replace(before, after), encoding='utf-8')

    def test_valid_contract(self):
        self.assertEqual(contract.check(self.root, self.project), contract.SHARED_SHA256)

    def test_missing_section_is_rejected(self):
        self.mutate('AGENTS.md', '## 3. GitHub task (TASK)', 'Task')
        with self.assertRaises(ValueError):
            contract.check(self.root, self.project)

    def test_shared_drift_even_in_both_copies_is_rejected(self):
        for name in ['AGENTS.md', '.github/DEVELOPMENT_INTERFACE.md']:
            self.mutate(name, 'Never use RDC', 'Sometimes use RDC')
        with self.assertRaisesRegex(ValueError, 'drift'):
            contract.check(self.root, self.project)

    def test_interface_link_is_required(self):
        self.mutate('AGENTS.md', '](.github/DEVELOPMENT_INTERFACE.md)', '](missing.md)')
        with self.assertRaises(ValueError):
            contract.check(self.root, self.project)

    def test_native_gate_cannot_disappear(self):
        required = contract.REQUIRED[self.project][-1]
        self.mutate('AGENTS.md', required, 'removed-native-requirement')
        with self.assertRaisesRegex(ValueError, 'mandatory'):
            contract.check(self.root, self.project)

    def test_wrong_product_is_rejected(self):
        self.mutate('AGENTS.md', '<!-- agent-project: ' + self.project + ' -->', '<!-- missing -->')
        with self.assertRaisesRegex(ValueError, 'identity'):
            contract.check(self.root, self.project)


if __name__ == '__main__':
    unittest.main()

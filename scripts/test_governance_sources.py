"""Consumer source-adoption integrity; no sync, subprocesses or network."""
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
RECORDS = {'.engineering-kit/source.json': '50e9158576f3c4802dd3960f82d04d7364097da1b27f593dcbca408c477c1893', '.project-template/source.json': '7ce7524de70159b102b0aee2772e4aa211a2427e75b9bcc6733b4f8ec3c41683'}
LOCAL_SHA256 = 'ee1e8ed04a86a8af42b7e6fecb30dbb3fa09973b9359bb7baced437813cec759'


def verify(root):
    for name, expected in RECORDS.items():
        path = root / name
        if any(p.is_symlink() for p in [path, *path.parents] if p != root.parent):
            raise ValueError("symlinked source record")
        raw = path.read_bytes()
        if hashlib.sha256(raw).hexdigest() != expected:
            raise ValueError("source record changed without review")
        record = json.loads(raw)
        if record['method'] != 'reviewed-public-source-migration' or len(record['files']) != 6:
            raise ValueError("invalid source adoption")
        for relative, source in record['files'].items():
            target = root / relative
            if any(p.is_symlink() for p in [target, *target.parents]):
                raise ValueError("symlinked managed source")
            if hashlib.sha256(target.read_bytes()).hexdigest() != source['sha256']:
                raise ValueError("managed source edited")
    if hashlib.sha256((root / 'AGENTS.local.md').read_bytes()).hexdigest() != LOCAL_SHA256:
        raise ValueError("native restrictions changed without review")
    if 'AGENTS.local.md' not in (root / 'AGENTS.md').read_text(encoding='utf-8'):
        raise ValueError("local instruction discovery missing")
    if hashlib.sha256((root / 'GOVERNANCE-NOTICE.txt').read_bytes()).hexdigest() != '3c995763395577bc7c6039c26b99152cbc9ddb01ccf2878d13c8dae88321ceec':
        raise ValueError("MIT notice missing or edited")


class GovernanceSources(unittest.TestCase):
    def test_pinned_sources_and_local_discovery(self):
        verify(ROOT)

    def test_native_ci_gates_retained(self):
        gates = {'ci.yml': ['ubuntu-latest, windows-latest', 'cargo fmt --all --check', 'cargo check --workspace --all-targets', 'cargo test --workspace', 'cargo clippy --workspace --all-targets --all-features -- -D warnings']}
        for workflow, commands in gates.items():
            text = (ROOT / '.github/workflows' / workflow).read_text(encoding='utf-8')
            for command in commands:
                with self.subTest(workflow=workflow, command=command):
                    self.assertIn(command, text)

    def test_canonical_bytes_keep_lf_on_windows(self):
        attributes = (ROOT / '.gitattributes').read_text(encoding='utf-8').splitlines()
        paths = [*RECORDS, 'AGENTS.local.md', 'GOVERNANCE-NOTICE.txt']
        for name in RECORDS:
            paths.extend(json.loads((ROOT / name).read_bytes())['files'])
        for name in paths:
            self.assertIn('/' + name + ' text eol=lf', attributes)

    def fixture(self, destination):
        for name in [*RECORDS, 'AGENTS.local.md', 'GOVERNANCE-NOTICE.txt']:
            p = destination / name
            p.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, p)
        for name in RECORDS:
            for relative in json.loads((ROOT / name).read_bytes())['files']:
                p = destination / relative
                p.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(ROOT / relative, p)

    def test_missing_edited_and_tampered_sources_fail(self):
        for kind in ['missing', 'edited', 'record', 'local', 'notice', 'notice-edited']:
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                self.fixture(root)
                target = root / ('AGENTS.local.md' if kind == 'local' else
                                 'GOVERNANCE-NOTICE.txt' if kind in ['notice', 'notice-edited'] else
                                 next(iter(RECORDS)) if kind == 'record' else 'AGENTS.md')
                if kind in ['missing', 'notice']:
                    target.unlink()
                else:
                    target.write_bytes(b'tampered')
                with self.assertRaises((ValueError, FileNotFoundError)):
                    verify(root)

    def test_symlinked_source_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.fixture(root)
            target = root / 'AGENTS.md'
            target.unlink()
            try:
                target.symlink_to(root / 'AGENTS.local.md')
            except OSError:
                self.skipTest('platform does not permit test symlinks')
            with self.assertRaises(ValueError):
                verify(root)


if __name__ == '__main__':
    unittest.main()

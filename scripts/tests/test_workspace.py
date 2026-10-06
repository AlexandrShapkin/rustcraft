"""Isolated Git fixture tests; never use a remembered owner checkout."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('workspace', Path(__file__).parents[1] / 'workspace.py')
workspace = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(workspace)


@unittest.skipUnless(shutil.which('git'), 'Git is required')
class WorkspaceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='rustcraft-workspace-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'checkout with spaces'
        self.root.mkdir()
        self.git('init', '--initial-branch=main')
        self.git('config', 'user.email', 'fixture@example.invalid')
        self.git('config', 'user.name', 'Workspace fixture')
        self.git('config', 'commit.gpgsign', 'false')
        self.git('config', 'core.hooksPath', str(self.root / 'no-hooks'))
        (self.root / 'docs').mkdir()
        (self.root / 'docs/stages.toml').write_text(
            'focus = "WF1"\n[[stage]]\nid = "WF1"\nstate = "active"\n', encoding='utf-8')

    def git(self, *args):
        return subprocess.check_output(['git', '-C', str(self.root), *args], text=True, stderr=subprocess.PIPE).strip()

    def commit(self):
        self.git('add', '.')
        self.git('commit', '-m', 'Fixture baseline')

    def test_root_nested_directory_clean_head_upstream_and_remote(self):
        self.commit()
        self.git('remote', 'add', 'origin', 'https://github.com/example/project.git')
        self.git('update-ref', 'refs/remotes/origin/main', 'HEAD')
        self.git('branch', '--set-upstream-to=origin/main')
        before = self.git('status', '--porcelain')
        status = workspace.collect(self.root / 'docs')
        self.assertEqual(status['repository_root'], str(self.root.resolve()))
        self.assertEqual(status['command_directory'], str((self.root / 'docs').resolve()))
        self.assertEqual(status['head'], self.git('rev-parse', 'HEAD'))
        self.assertEqual(status['branch'], 'main')
        self.assertEqual(status['upstream'], 'origin/main')
        self.assertEqual(status['remote_url'], 'https://github.com/example/project.git')
        self.assertEqual((status['worktree'], status['focus'], status['stage_state']), ('clean', 'WF1', 'active'))
        self.assertEqual(before, self.git('status', '--porcelain'))

    def test_untracked_and_modified_files_are_dirty(self):
        self.commit()
        p = self.root / 'scratch'
        p.write_text('untracked', encoding='utf-8')
        self.assertEqual(workspace.collect(self.root)['worktree'], 'dirty')
        p.unlink()
        (self.root / 'docs/stages.toml').write_text('focus = "WF1"\n[[stage]]\nid="WF1"\nstate="closed"\n', encoding='utf-8')
        self.assertEqual(workspace.collect(self.root)['worktree'], 'dirty')

    def test_detached_unborn_missing_upstream_and_remote(self):
        self.assertEqual(workspace.collect(self.root)['head'], '(unborn)')
        self.commit()
        self.git('checkout', '--detach')
        status = workspace.collect(self.root)
        self.assertEqual(status['branch'], '(detached)')
        self.assertIsNone(status['upstream'])
        self.assertIsNone(status['remote_url'])

    def test_invalid_registry_and_outside_repository_fail(self):
        with self.assertRaises(workspace.WorkspaceError):
            workspace.collect(Path(self.temp.name))
        for text in ['not toml', 'focus="OTHER"\n[[stage]]\nid="WF1"\nstate="active"\n']:
            (self.root / 'docs/stages.toml').write_text(text, encoding='utf-8')
            with self.assertRaisesRegex(workspace.WorkspaceError, 'focused stage'):
                workspace.collect(self.root)

    def test_cli_json_uses_caller_checkout_and_errors_are_clear(self):
        self.commit()
        output = io.StringIO()
        with patch.object(workspace.Path, 'cwd', return_value=self.root), contextlib.redirect_stdout(output):
            self.assertEqual(workspace.main(['--json']), 0)
        self.assertEqual(json.loads(output.getvalue())['repository_root'], str(self.root.resolve()))
        output = io.StringIO()
        with patch.object(workspace.Path, 'cwd', return_value=Path(self.temp.name)), contextlib.redirect_stderr(output):
            self.assertEqual(workspace.main([]), 1)
        self.assertIn('Workspace status error:', output.getvalue())

    def test_https_remote_credentials_are_never_reported(self):
        self.commit()
        self.git('remote', 'add', 'origin', 'https://owner:secret@github.com/example/project.git?token=secret#secret')
        status = workspace.collect(self.root)
        self.assertEqual(status['remote_url'], 'https://github.com/example/project.git?redacted')
        self.assertNotIn('secret', json.dumps(status))
        self.assertEqual(workspace.public_remote('git@github.com:example/project.git'), 'git@github.com:example/project.git')

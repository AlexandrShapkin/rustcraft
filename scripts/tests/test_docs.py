"""Fast documentation-tool checks; all mutations stay inside temporary fixtures."""
import contextlib
import importlib.util
import io
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('docs_tool', Path(__file__).parents[1] / 'docs.py')
docs = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(docs)


class DocsTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for name in ('docs/stages', 'docs/templates', 'crates/example'):
            (self.root / name).mkdir(parents=True)
        self.put('ROADMAP.md', '# Roadmap navigation\n\n- [Canonical overview](docs/ROADMAP.md)\n- [Machine-readable stage registry](docs/stages.toml)\n- [Stage contracts and generated index](docs/stages/INDEX.md)\n')
        self.put('docs/ROADMAP.md', 'Manual preamble.\n' + docs.BEGIN + '\n' + docs.END + '\nManual epilogue.\n')
        self.put('docs/DECISIONS.md', '# Decisions\n\n## D-001 — Fixture choice\n\nOld rationale.\n')
        self.put('docs/ARCHITECTURE.md', '# Manual architecture\n\nUnrelated owner input.\n')
        self.put('docs/DEFECTS.md', '# Issue policy\n')
        self.put('docs/WORKFLOW.md', '# Workflow\n')
        template = '# <ID> — <Title>\n\n' + '\n\n'.join('## ' + h + '\n\nAuthor '+h+'.' for h in docs.HEADINGS) + '\n'
        self.put('docs/templates/STAGE.md', template)
        for name in ('REPORT', 'DECISION'):
            self.put(f'docs/templates/{name}.md', '# Template\n')
        rows = []
        for sid in ('BG1', 'DX2'):
            self.put(f'docs/stages/{sid}.md', template.replace('<ID>', sid).replace('<Title>', sid))
            rows.append(f'[[stage]]\nid = "{sid}"\ntitle = "{sid}"\nstate = "planned"\ncontract = "docs/stages/{sid}.md"\nissue_label = "stage:{sid}"\nrelevant_docs = ["docs/ARCHITECTURE.md"]\ndecisions = ["D-001"]\nsource_areas = ["crates/example"]\n')
        self.put('docs/stages.toml', 'version = 1\nfocus = "BG1"\nsequence = ["BG1", "DX2"]\n\n'+'\n'.join(rows))
        docs.sync(self.root)

    def put(self, name, text):
        docs.write(self.root / name, text)

    def mutate(self, old, new):
        p = self.root / 'docs/stages.toml'
        docs.write(p, docs.read(p).replace(old, new))

    def quiet(self, fn, *args):
        with contextlib.redirect_stdout(io.StringIO()):
            return fn(*args)

    def test_duplicate_stage_rejected(self):
        self.mutate('id = "DX2"', 'id = "BG1"')
        with self.assertRaisesRegex(docs.DocsError, 'Duplicate stage ID'):
            docs.load(self.root)

    def test_duplicate_sequence_rejected(self):
        self.mutate('["BG1", "DX2"]', '["BG1", "BG1"]')
        with self.assertRaisesRegex(docs.DocsError, 'Duplicate sequence'):
            docs.load(self.root)

    def test_unknown_focus_rejected(self):
        self.mutate('focus = "BG1"', 'focus = "UNKNOWN"')
        with self.assertRaisesRegex(docs.DocsError, 'Focus'):
            docs.load(self.root)

    def test_active_focus_and_count(self):
        self.mutate('id = "DX2"\ntitle = "DX2"\nstate = "planned"', 'id = "DX2"\ntitle = "DX2"\nstate = "active"')
        with self.assertRaisesRegex(docs.DocsError, 'Active stage must be focus'):
            docs.load(self.root)
        self.mutate('state = "planned"', 'state = "active"')
        with self.assertRaisesRegex(docs.DocsError, 'At most one'):
            docs.load(self.root)

    def test_insert_after_existing_preserves_unrelated_files(self):
        before = {p.relative_to(self.root): p.read_bytes() for p in self.root.rglob('*') if p.is_file()}
        self.quiet(docs.new_stage, self.root, 'TESTX', 'Temporary Documentation Test Stage', 'BG1')
        plan, by = docs.load(self.root)
        self.assertEqual(plan['sequence'], ['BG1', 'TESTX', 'DX2'])
        self.assertEqual(by['TESTX']['state'], 'planned')
        self.assertEqual(by['TESTX']['issue_label'], 'stage:TESTX')
        self.assertIn('Temporary Documentation Test Stage', docs.read(self.root/'docs/stages/TESTX.md'))
        self.quiet(docs.check, self.root)
        changed = {p for p, data in before.items() if (self.root/p).read_bytes() != data}
        self.assertEqual(changed, {Path('docs/stages.toml'), Path('docs/ROADMAP.md'), Path('docs/stages/INDEX.md')})
        self.assertEqual({p.relative_to(self.root) for p in self.root.rglob('*') if p.is_file()} - before.keys(), {Path('docs/stages/TESTX.md')})
        self.assertIn('Manual preamble.', docs.read(self.root/'docs/ROADMAP.md'))
        self.assertTrue(docs.read(self.root/'docs/ROADMAP.md').endswith('Manual epilogue.\n'))

    def test_unknown_insertion_and_duplicate_do_not_mutate(self):
        before = docs.read(self.root/'docs/stages.toml')
        for sid, after in [('NEW', 'MISSING'), ('BG1', 'DX2'), ('../BAD', 'BG1')]:
            with self.assertRaises(docs.DocsError):
                docs.new_stage(self.root, sid, 'Title', after)
        self.assertEqual(before, docs.read(self.root/'docs/stages.toml'))

    def test_sync_idempotence_and_stale_detection(self):
        paths = [self.root/'docs/ROADMAP.md', self.root/'docs/stages/INDEX.md']
        before = [(p.read_bytes(), p.stat().st_mtime_ns) for p in paths]
        docs.sync(self.root); docs.sync(self.root)
        self.assertEqual(before, [(p.read_bytes(), p.stat().st_mtime_ns) for p in paths])
        self.mutate('title = "DX2"', 'title = "Changed"')
        with self.assertRaisesRegex(docs.DocsError, 'index stale'):
            docs.check(self.root)
        docs.sync(self.root); self.quiet(docs.check, self.root)

    def test_broken_and_wrong_case_links(self):
        for target in ('missing.md', 'architecture.md'):
            self.put('docs/other.md', f'[Link]({target})\n')
            with self.assertRaisesRegex(docs.DocsError, 'Missing or wrong-case'):
                docs.check_links(self.root)

    def test_links_with_parent_spaces_and_reference_style(self):
        self.put('docs/space name.md', '# Fine\n')
        self.put('docs/other.md', '[A](<space name.md>)\n[B][ref]\n[ref]: ARCHITECTURE.md\n[C](../ROADMAP.md)\n')
        docs.check_links(self.root)

    def test_duplicate_decision_rejected(self):
        self.put('docs/DECISIONS.md', '## D-001 — First\n## D-001 — Second\n')
        with self.assertRaisesRegex(docs.DocsError, 'Duplicate decision ID'):
            docs.load(self.root)

    def test_offline_context_never_calls_github(self):
        output = io.StringIO()
        with patch.object(docs, 'git', return_value='fixture'), patch.object(docs.subprocess, 'run', side_effect=AssertionError('Network attempted')), contextlib.redirect_stdout(output):
            docs.context(self.root, offline=True)
        self.assertIn('BG1', output.getvalue())
        self.assertIn('GitHub skipped', output.getvalue())
        self.assertLess(len(output.getvalue()), 2000)

    def test_unavailable_github_is_nonfatal(self):
        for failure in (FileNotFoundError(), subprocess.TimeoutExpired('gh', 8), subprocess.CalledProcessError(1, 'gh')):
            with patch.object(docs, 'git', return_value='fixture'), patch.object(docs.subprocess, 'run', side_effect=failure), contextlib.redirect_stdout(io.StringIO()) as output:
                docs.context(self.root)
            self.assertIn('GitHub issues unavailable', output.getvalue())

    def test_unsafe_source_path_and_missing_doc(self):
        self.mutate('crates/example', '../escape')
        with self.assertRaises(docs.DocsError):
            docs.load(self.root)

    def test_missing_heading_rejected(self):
        self.put('docs/stages/BG1.md', '# BG1\n')
        with self.assertRaisesRegex(docs.DocsError, 'headings'):
            docs.check(self.root)

    def test_root_pointer_and_snapshot_rejected(self):
        self.put('ROADMAP.md', 'F1 active\n')
        with self.assertRaisesRegex(docs.DocsError, 'pure stable pointer'):
            docs.check(self.root)

    def test_snapshot_rejected(self):
        self.put('PROJECT_TREE.txt', 'stale tree')
        with self.assertRaisesRegex(docs.DocsError, 'PROJECT_TREE'):
            docs.check(self.root)

    def test_source_glob_and_empty_glob(self):
        self.mutate('crates/example', 'crates/*')
        docs.load(self.root)
        self.mutate('crates/*', 'crates/missing*')
        with self.assertRaisesRegex(docs.DocsError, 'Empty source glob'):
            docs.load(self.root)

    def test_invalid_state_and_issue_label(self):
        self.mutate('state = "planned"', 'state = "doing"')
        with self.assertRaisesRegex(docs.DocsError, 'Invalid state'):
            docs.load(self.root)
        self.mutate('state = "doing"', 'state = "planned"')
        self.mutate('issue_label = "stage:BG1"', 'issue_label = "historical:BG1"')
        with self.assertRaisesRegex(docs.DocsError, 'Invalid issue label'):
            docs.load(self.root)

    def test_non_ascii_stage_title(self):
        self.quiet(docs.new_stage, self.root, 'TESTX', 'Документация 🚀', 'BG1')
        self.assertEqual(docs.load(self.root)[1]['TESTX']['title'], 'Документация 🚀')

    def test_image_file_target_checked(self):
        self.put('docs/image.md', '![Evidence](missing.png)\n')
        with self.assertRaisesRegex(docs.DocsError, 'Missing or wrong-case'):
            docs.check_links(self.root)

    def test_new_stage_rollback_on_sync_failure(self):
        before = {p: p.read_bytes() for p in [self.root/'docs/stages.toml', self.root/'docs/ROADMAP.md', self.root/'docs/stages/INDEX.md']}
        with patch.object(docs, 'sync', side_effect=OSError('Injected write failure')):
            with self.assertRaises(OSError):
                self.quiet(docs.new_stage, self.root, 'TESTX', 'Title', 'BG1')
        self.assertFalse((self.root/'docs/stages/TESTX.md').exists())
        self.assertTrue(all(p.read_bytes() == data for p, data in before.items()))


if __name__ == '__main__':
    unittest.main()

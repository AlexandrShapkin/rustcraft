"""Offline instruction consistency only; never executes product checks or Git."""
import argparse
import hashlib
from pathlib import Path

START = '<!-- shared-agent:start -->\n'
END = '<!-- shared-agent:end -->'
SHARED_SHA256 = '4483730731b7b2fcfc6152760efe099d1c7ece62ccc4c476ccec242077620e5a'
SECTIONS = (
    '1. Purpose and authority', '2. Repository start (START)',
    '3. GitHub task (TASK)', '4. Necessary context (CONTEXT)',
    '5. Planning and implementation (IMPLEMENT)', '6. Verification and quality (VERIFY)',
    '7. PR, CI, review and acceptance (PR / REVIEW)',
    '8. Temporary resources (CLEANUP)', '9. Final evidence (REPORT)',
    '10. Product-specific rules',
)
REQUIRED = {
    'codex-smart': ('just context [STAGE]', 'docs/workflow.md', 'just docs-check',
                    'just check', 'just security', 'PR #125', 'PR #164'),
    'relayforge': ('docs/development-without-rdc.md', 'docs/security.md', 'Rustfmt',
                   'Clippy', 'cargo test --workspace --locked', 'recovery tests',
                   'isolated release build', 'offline MCP smoke', 'CARGO_TARGET_DIR',
                   'P0 runtime/build boundary'),
    'assistant-hub': ('docs/workflow.md', '3.11–3.13',
                      'PYTHONPATH=src python -m unittest discover -s tests -v',
                      'python -m compileall -q src tests scripts',
                      'PYTHONPATH=src python -m assistant_hub doctor',
                      'python scripts/check_public_tree.py --include-untracked',
                      'node --test web/tests/*.test.mjs', 'git diff --check'),
    'rustcraft': ('just codex-context [STAGE]', 'docs/INDEX.md', 'just ci',
                  'Ubuntu and Windows', 'reference-study', 'engine/game policy'),
}


def block(text):
    if text.count(START) != 1 or text.count(END) != 1:
        raise ValueError('exactly one shared block required')
    before, after = text.split(START)
    common, tail = after.split(END)
    if END in before:
        raise ValueError('invalid shared block order')
    return common, tail


def check(root, project):
    root = Path(root)
    agents = (root / 'AGENTS.md').read_text(encoding='utf-8')
    interface = (root / '.github/DEVELOPMENT_INTERFACE.md').read_text(encoding='utf-8')
    common, product = block(agents)
    normative, _ = block(interface)
    if common != normative or hashlib.sha256(common.encode()).hexdigest() != SHARED_SHA256:
        raise ValueError('shared agent block drift')
    headings = [line[3:] for line in common.splitlines() if line.startswith('## ')]
    if tuple(headings) != SECTIONS:
        raise ValueError('required sections missing or reordered')
    if '[.github/DEVELOPMENT_INTERFACE.md](.github/DEVELOPMENT_INTERFACE.md)' not in common:
        raise ValueError('shared interface link missing')
    if '<!-- agent-project: ' + project + ' -->' not in product:
        raise ValueError('product identity mismatch')
    for required in REQUIRED[project]:
        if required not in product:
            raise ValueError('mandatory product instruction missing: ' + required)
    return hashlib.sha256(common.encode()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--project', required=True, choices=REQUIRED)
    parser.add_argument('--peer', type=Path, action='append', default=[])
    args = parser.parse_args()
    try:
        digest = check(args.root, args.project)
        for peer in args.peer:
            text = (peer / 'AGENTS.md').read_text(encoding='utf-8')
            products = [p for p in REQUIRED if '<!-- agent-project: ' + p + ' -->' in text]
            if len(products) != 1 or check(peer, products[0]) != digest:
                raise ValueError('peer contract mismatch')
    except (OSError, ValueError, KeyError) as exc:
        parser.exit(1, 'FAIL: ' + str(exc) + '\n')
    print('PASS: shared agent contract ' + digest)


if __name__ == '__main__':
    main()

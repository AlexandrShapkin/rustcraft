#!/usr/bin/env python3
"""Small, offline-first documentation navigation tool (Python 3.11+, stdlib)."""
import argparse
import glob
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib
from urllib.parse import unquote, urlsplit

STATES = {'planned', 'active', 'blocked', 'closed', 'inactive'}
ID = re.compile(r'[A-Z][A-Z0-9]*(?:\.[0-9]+)?\Z')
HEADINGS = ('Goal', 'Context', 'Current state', 'Target state', 'Scope', 'Invariants',
            'Out of scope', 'Implementation constraints', 'Acceptance', 'Validation',
            'Documentation updates', 'Issue reconciliation', 'Closeout')
BEGIN = '<!-- BEGIN GENERATED STAGE PLAN -->'
END = '<!-- END GENERATED STAGE PLAN -->'


class DocsError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise DocsError(message)


def read(path):
    return path.read_text(encoding='utf-8')


def write(path, text):
    path.write_text(text, encoding='utf-8', newline='\n')


def local_path(root, value):
    """Reject escapes and wrong-case paths even on Windows."""
    p = Path(value)
    require(not p.is_absolute() and '..' not in p.parts, f'Unsafe path: {value}')
    current = root
    for part in p.parts:
        require(current.is_dir() and part in {x.name for x in current.iterdir()},
                f'Missing or wrong-case path: {value}')
        current = current / part
    require(current.resolve().is_relative_to(root.resolve()), f'Escaping path: {value}')
    return current


def decisions(root):
    result = {}
    for key, title in re.findall(r'^#{1,6} (D-\d{3}) — (.+)$', read(root / 'docs/DECISIONS.md'), re.M):
        require(key not in result, f'Duplicate decision ID: {key}')
        result[key] = title
    return result


def load(root):
    try:
        plan = tomllib.loads(read(root / 'docs/stages.toml'))
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise DocsError(f'Invalid registry: {exc}') from exc
    require(type(plan.get('version')) is int and plan.get('version') == 1, 'Unsupported registry version')
    seq, rows = plan.get('sequence'), plan.get('stage')
    require(isinstance(seq, list) and all(isinstance(x, str) for x in seq), 'sequence must be an ID list')
    require(isinstance(rows, list) and rows, 'stage entries required')
    require(len(seq) == len(set(seq)), 'Duplicate sequence ID')
    by = {}
    ds = decisions(root)
    for row in rows:
        require(isinstance(row, dict), 'Each stage must be a table')
        sid = row.get('id')
        require(isinstance(sid, str) and ID.fullmatch(sid), f'Invalid stage ID: {sid}')
        require(sid not in by, f'Duplicate stage ID: {sid}')
        by[sid] = row
        require(isinstance(row.get('state'), str) and row.get('state') in STATES, f'Invalid state: {sid}')
        require(isinstance(row.get('title'), str) and row['title'].strip()
                and not any(c in row['title'] for c in '\n\r|'), f'Invalid title: {sid}')
        require(row.get('contract') == f'docs/stages/{sid}.md', f'Invalid contract path: {sid}')
        require(local_path(root, row['contract']).is_file(), f'Missing contract: {sid}')
        require(row.get('issue_label') == f'stage:{sid}', f'Invalid issue label: {sid}')
        for field in ('relevant_docs', 'decisions', 'source_areas'):
            values = row.get(field)
            require(isinstance(values, list) and all(isinstance(x, str) for x in values), f'Invalid {field}: {sid}')
            require(len(values) == len(set(values)), f'Duplicate {field}: {sid}')
        for doc in row['relevant_docs']:
            require(local_path(root, doc).is_file(), f'Missing document: {doc}')
        for key in row['decisions']:
            require(key in ds, f'Unknown decision: {sid}: {key}')
        for area in row['source_areas']:
            if glob.has_magic(area):
                require(not Path(area).is_absolute() and '..' not in Path(area).parts, f'Unsafe source glob: {area}')
                matches = glob.glob(area, root_dir=root, recursive=True)
                require(matches, f'Empty source glob: {area}')
                for match in matches:
                    local_path(root, match)
            else:
                local_path(root, area)
    require(set(seq) == set(by), 'Sequence must contain every registry stage exactly once')
    require(isinstance(plan.get('focus'), str) and plan.get('focus') in by, 'Focus must reference an existing stage')
    active = [s['id'] for s in rows if s['state'] == 'active']
    require(len(active) <= 1, 'At most one stage may be active')
    require(not active or active[0] == plan['focus'], 'Active stage must be focus')
    return plan, by


def views(plan, by):
    focus = plan['focus']
    block = f'{BEGIN}\n\nCurrent focus: **{focus}**. Execution order and state are generated from [stages.toml](stages.toml).\n\n| Stage | State | Contract |\n| --- | --- | --- |\n'
    index = '# Stage index\n\nGENERATED — DO NOT EDIT BY HAND. Source: [stages.toml](../stages.toml).\n\n'
    index += f'Current focus: **{focus}**. List order is authorized execution order; it does not authorize starting work.\n\n| Stage | Title | State | Contract | Issue label |\n| --- | --- | --- | --- | --- |\n'
    for sid in plan['sequence']:
        r = by[sid]
        block += f'| {sid} | {r["state"]} | [{r["title"]}](stages/{sid}.md) |\n'
        index += f'| {sid} | {r["title"]} | {r["state"]} | [{sid}]({sid}.md) | `{r["issue_label"]}` |\n'
    return block + '\n' + END, index


def roadmap_view(root, block):
    text = read(root / 'docs/ROADMAP.md')
    require(text.count(BEGIN) == 1 and text.count(END) == 1, 'ROADMAP must have exactly one generated marker pair')
    start, end = text.index(BEGIN), text.index(END)
    require(start < end, 'Reversed ROADMAP markers')
    return text[:start] + block + text[end + len(END):]


def sync(root):
    plan, by = load(root)
    block, index = views(plan, by)
    roadmap = roadmap_view(root, block)
    for path, text in ((root / 'docs/ROADMAP.md', roadmap), (root / 'docs/stages/INDEX.md', index)):
        if not path.exists() or read(path) != text:
            write(path, text)


def markdown_files(root):
    # Project-authored documentation only; never traverse target/private reference trees.
    paths = list(root.glob('*.md'))
    for directory in ('docs', '.agents', '.github', 'reference/notes'):
        paths.extend((root / directory).rglob('*.md'))
    paths.append(root / 'reference/SOURCES.md')
    return sorted(set(p for p in paths if p.is_file()))


def check_links(root):
    count = 0
    for p in markdown_files(root):
        text = re.sub(r'(?ms)^```.*?^```[^\n]*', '', read(p))
        targets = re.findall(r'\[[^\]\n]+\]\((<[^>]+>|[^\s)]+)(?:\s+"[^"]*")?\)', text)
        targets += re.findall(r'^\[[^\]]+\]:\s*(<[^>]+>|\S+)', text, re.M)
        for target in targets:
            target = target.strip('<>')
            u = urlsplit(target)
            if u.scheme or not u.path:
                continue
            resolved = p.parent / unquote(u.path)
            require(resolved.resolve().is_relative_to(root.resolve()), f'Escaping link: {p.relative_to(root)}: {target}')
            # resolve() normalizes '..'; exact case is checked against authored components below.
            authored = Path(os.path.normpath(str(resolved.relative_to(root))))
            local_path(root, authored)
            count += 1
    return count


def check(root):
    plan, by = load(root)
    block, index = views(plan, by)
    require(read(root / 'docs/stages/INDEX.md') == index, 'Stage index stale; run docs-sync')
    require(read(root / 'docs/ROADMAP.md') == roadmap_view(root, block), 'ROADMAP generated plan stale; run docs-sync')
    for row in by.values():
        text = read(root / row['contract'])
        found = set(re.findall(r'^## (.+)$', text, re.M))
        require(set(HEADINGS) <= found, f'Missing required contract headings: {row["id"]}')
        require(not re.search(r'^Status:', text, re.M), f'Mutable Status in contract: {row["id"]}')
    for name in ('STAGE', 'REPORT', 'DECISION'):
        require((root / f'docs/templates/{name}.md').is_file(), f'Missing template: {name}')
    expected = '# Roadmap navigation\n\n- [Canonical overview](docs/ROADMAP.md)\n- [Machine-readable stage registry](docs/stages.toml)\n- [Stage contracts and generated index](docs/stages/INDEX.md)\n'
    require(read(root / 'ROADMAP.md') == expected, 'Root ROADMAP must remain a pure stable pointer')
    require(not (root / 'PROJECT_TREE.txt').exists(), 'PROJECT_TREE.txt must not be committed/generated in the root')
    count = check_links(root)
    print(f'Docs check passed: {len(by)} stages; {len(decisions(root))} decisions; {count} local file links; generated views synchronized.')


def git(root, *args):
    try:
        return subprocess.check_output(['git', '-C', str(root), *args], text=True, stderr=subprocess.DEVNULL, timeout=5).strip()
    except (OSError, subprocess.SubprocessError):
        return 'unavailable'


def context(root, sid=None, offline=False):
    plan, by = load(root)
    sid = sid or plan['focus']
    require(sid in by, f'Unknown stage: {sid}')
    r = by[sid]
    status = git(root, 'status', '--short')
    print('HEAD\n  ' + git(root, 'rev-parse', 'HEAD'))
    print('WORKTREE\n  ' + ('clean' if not status else f'{len(status.splitlines())} changed/untracked paths' if status != 'unavailable' else status))
    print(f'STAGE\n  {sid} — {r["title"]}\n  state: {r["state"]}; focus: {plan["focus"]}')
    print('CONTRACT\n  ' + r['contract'])
    print('READ NEXT\n' + '\n'.join('  ' + d for d in r['relevant_docs']))
    ds = decisions(root)
    print('DECISIONS\n' + '\n'.join(f'  {k} — {ds[k]}' for k in r['decisions']))
    print('SOURCE AREAS (navigation hints)\n' + '\n'.join('  ' + a for a in r['source_areas']))
    print('ISSUE LABEL\n  ' + r['issue_label'])
    print('OPEN ISSUES')
    if offline:
        print('  GitHub skipped (--offline); retry online for current issue state.')
    else:
        try:
            result = subprocess.run(['gh', 'issue', 'list', '--repo', 'AlexandrShapkin/rustcraft', '--state', 'open',
                '--label', r['issue_label'], '--limit', '10', '--json', 'number,title'], capture_output=True, text=True, timeout=8, check=True)
            issues = json.loads(result.stdout)
            for issue in issues:
                print(f'  #{issue["number"]} {issue["title"]}')
            print('  No labelled open issues.' if not issues else '  Up to 10 shown; query the label for complete reconciliation.')
        except (OSError, subprocess.SubprocessError, ValueError, KeyError):
            print('  GitHub issues unavailable; use repository issue policy / retry online.')
    print('NEXT\n  Read the contract, then only task-relevant pointers; docs/INDEX.md defines precedence.\n  just docs-check; git status --short; focused validation from contract.\n  This context does not authorize activation or implementation.')


def new_stage(root, sid, title, after):
    require(ID.fullmatch(sid), f'Invalid new stage ID: {sid}')
    plan, by = load(root)
    require(sid not in by, f'Duplicate stage ID: {sid}')
    require(after in by, f'Unknown insertion target: {after}')
    require(title.strip() and not any(c in title for c in '\r\n|'), 'Invalid title')
    path = root / f'docs/stages/{sid}.md'
    require(not path.exists(), f'Contract already exists: {path.name}')
    check(root)  # Do not compound existing broken planning.
    registry = root / 'docs/stages.toml'
    original = read(registry)
    seq = list(plan['sequence']); seq.insert(seq.index(after) + 1, sid)
    require(len(re.findall(r'^sequence\s*=\s*\[.*?\]', original, re.M | re.S)) == 1, 'Expected one sequence list')
    updated = re.sub(r'^sequence\s*=\s*\[.*?\]', 'sequence = ' + json.dumps(seq), original, count=1, flags=re.M | re.S)
    updated += f'\n[[stage]]\nid = {json.dumps(sid)}\ntitle = {json.dumps(title, ensure_ascii=False)}\nstate = "planned"\ncontract = "docs/stages/{sid}.md"\nissue_label = "stage:{sid}"\nrelevant_docs = []\ndecisions = []\nsource_areas = []\n'
    contract = read(root / 'docs/templates/STAGE.md').replace('<ID>', sid).replace('<Title>', title)
    # Roll back our own files on validation/write failure; never touch unrelated docs.
    generated = [root / 'docs/ROADMAP.md', root / 'docs/stages/INDEX.md']
    backups = [(p, p.read_bytes()) for p in generated]
    try:
        write(path, contract); write(registry, updated); sync(root); check(root)
    except Exception:
        write(registry, original)
        path.unlink(missing_ok=True)
        for p, data in backups:
            p.write_bytes(data)
        raise
    print(f'Created {sid} after {after}. Next: author its contract/navigation metadata; review diff and docs-check.\nNo GitHub label, commit, push or activation was created.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parent.parent, help=argparse.SUPPRESS)
    subs = parser.add_subparsers(dest='command', required=True)
    for command in ('check', 'sync'):
        subs.add_parser(command)
    p = subs.add_parser('context'); p.add_argument('stage', nargs='?'); p.add_argument('--offline', action='store_true')
    p = subs.add_parser('new-stage'); p.add_argument('id'); p.add_argument('title'); p.add_argument('--after', required=True)
    args = parser.parse_args()
    try:
        root = args.root.resolve()
        if args.command == 'check': check(root)
        elif args.command == 'sync': sync(root)
        elif args.command == 'context': context(root, args.stage, args.offline)
        else: new_stage(root, args.id, args.title, args.after)
    except (DocsError, OSError) as exc:
        print(f'docs: {exc}', file=sys.stderr); return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())

#!/usr/bin/env python3
"""Read-only checkout/stage status from the caller's directory (Python 3.11+, stdlib)."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import tomllib
from urllib.parse import urlsplit, urlunsplit


class WorkspaceError(ValueError):
    pass


def git(directory, *args, optional=False):
    try:
        result = subprocess.run(['git', '--no-optional-locks', '-C', str(directory), *args],
                                capture_output=True, text=True, encoding='utf-8', errors='replace')
    except OSError as error:
        raise WorkspaceError('Git is required to inspect this checkout.') from error
    if result.returncode:
        if optional:
            return None
        raise WorkspaceError('Cannot discover a Git worktree from the current directory.')
    return result.stdout.strip()


def public_remote(url):
    """Status must not expose credentials embedded in HTTPS remote URLs."""
    if not url:
        return None
    if '://' in url:
        parsed = urlsplit(url)
        host = parsed.netloc.rsplit('@', 1)[-1]
        return urlunsplit((parsed.scheme, host, parsed.path,
                          'redacted' if parsed.query else '', ''))
    return url


def collect(directory=None):
    directory = Path(directory or Path.cwd()).resolve()
    root = Path(git(directory, 'rev-parse', '--show-toplevel')).resolve()
    head = git(root, 'rev-parse', '--verify', 'HEAD', optional=True)
    branch = git(root, 'symbolic-ref', '--quiet', '--short', 'HEAD', optional=True)
    upstream = git(root, 'rev-parse', '--abbrev-ref', '--symbolic-full-name',
                   '@{upstream}', optional=True) if branch and head else None
    changes = git(root, 'status', '--porcelain=v1', '--untracked-files=normal')
    remote_name = git(root, 'config', f'branch.{branch}.remote', optional=True) if branch else None
    remote_name = remote_name or 'origin'
    remote = git(root, 'remote', 'get-url', remote_name, optional=True)
    if remote is None and remote_name != 'origin':
        remote_name = 'origin'
        remote = git(root, 'remote', 'get-url', remote_name, optional=True)
    try:
        registry = tomllib.loads((root / 'docs/stages.toml').read_text(encoding='utf-8'))
        focus = registry['focus']
        stages = [stage for stage in registry['stage'] if stage['id'] == focus]
        if len(stages) != 1 or not isinstance(focus, str):
            raise ValueError('unknown or duplicate focus')
        state = stages[0]['state']
        if state not in {'planned', 'active', 'blocked', 'closed', 'inactive'}:
            raise ValueError('invalid state')
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise WorkspaceError(f'Cannot read focused stage from {root / "docs/stages.toml"}; run just docs-check.') from error
    return {'repository_root': str(root), 'command_directory': str(directory),
            'branch': branch or '(detached)', 'head': head or '(unborn)',
            'upstream': upstream, 'worktree': 'dirty' if changes else 'clean',
            'remote_name': remote_name if remote else None,
            'remote_url': public_remote(remote), 'focus': focus, 'stage_state': state}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--json', action='store_true', help='machine-readable status')
    args = parser.parse_args(argv)
    try:
        status = collect()
    except WorkspaceError as error:
        print(f'Workspace status error: {error}', file=sys.stderr)
        return 1
    if args.json:
        print(json.dumps(status, indent=2, ensure_ascii=False))
    else:
        for key, value in status.items():
            print(f'{key}: {value if value is not None else "(none)"}')
    return 0


if __name__ == '__main__':
    sys.exit(main())

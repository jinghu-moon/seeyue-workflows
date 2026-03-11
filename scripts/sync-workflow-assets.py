from __future__ import annotations

import argparse
import filecmp
import json
import shutil
from pathlib import Path


def load_manifest(source_root: Path) -> dict:
    manifest_path = source_root / 'sync-manifest.json'
    return json.loads(manifest_path.read_text(encoding='utf-8'))


def iter_files(path: Path):
    if path.is_file():
        yield path
        return
    for file_path in sorted(p for p in path.rglob('*') if p.is_file()):
        yield file_path


def compare_file(src: Path, dst: Path) -> bool:
    if not dst.exists() or not dst.is_file():
        return False
    return filecmp.cmp(src, dst, shallow=False)


def copy_entry(src_root: Path, dst_root: Path, entry: dict, check_only: bool):
    entry_type = entry['type']
    src = src_root / entry['source']
    dst = dst_root / entry['target']
    changes = []

    if entry_type == 'file':
        same = compare_file(src, dst)
        if not same:
            changes.append((src, dst))
            if not check_only:
                dst.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(src, dst)
        return changes

    if entry_type == 'dir':
        for src_file in iter_files(src):
            relative = src_file.relative_to(src)
            dst_file = dst / relative
            same = compare_file(src_file, dst_file)
            if not same:
                changes.append((src_file, dst_file))
                if not check_only:
                    dst_file.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(src_file, dst_file)
        return changes

    raise ValueError(f'unsupported entry type: {entry_type}')


def main() -> int:
    parser = argparse.ArgumentParser(description='Sync workflow assets from seeyue-workflows to a target repo.')
    parser.add_argument('--target-root', required=True, help='Path to the target repository root.')
    parser.add_argument('--check', action='store_true', help='Only check drift, do not copy files.')
    args = parser.parse_args()

    source_root = Path(__file__).resolve().parents[1]
    target_root = Path(args.target_root).resolve()
    manifest = load_manifest(source_root)

    all_changes = []
    for entry in manifest.get('entries', []):
        all_changes.extend(copy_entry(source_root, target_root, entry, args.check))

    mode = 'CHECK' if args.check else 'APPLY'
    print(f'[workflow-sync] mode={mode} changes={len(all_changes)}')
    for src, dst in all_changes:
        print(f'[workflow-sync] {src.relative_to(source_root).as_posix()} -> {dst.relative_to(target_root).as_posix()}')

    if args.check and all_changes:
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

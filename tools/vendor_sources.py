"""Tracked local dependency inputs shared by native, portable and replay receipts."""
import os
from pathlib import Path


def vendor_source_paths(root: Path) -> list[Path]:
    base = root / 'vendor/java_regex'
    if not os.path.lexists(base):
        return []
    if base.is_symlink() or not base.is_dir():
        raise ValueError('vendored source root must be a regular directory')
    paths = []
    excluded = {'target', '.git', '.cache', 'node_modules'}
    for directory, folders, names in os.walk(base, topdown=True, followlinks=False):
        folders[:] = sorted(name for name in folders if name not in excluded)
        for name in folders:
            if (Path(directory) / name).is_symlink():
                raise ValueError('vendored source directory symlink')
        for name in sorted(names):
            path = Path(directory) / name
            if path.is_symlink() or not path.is_file():
                raise ValueError('vendored source input must be a regular file')
            paths.append(path)
    return sorted(paths)

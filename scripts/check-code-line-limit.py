#!/usr/bin/env python3
from __future__ import annotations

import argparse
from pathlib import Path
import sys


DEFAULT_LIMIT = 500
DEFAULT_EXTENSIONS = {".rs", ".py", ".sh", ".js", ".jsx", ".ts", ".tsx"}
DEFAULT_EXCLUDE_DIRS = {".git", "target", "node_modules", ".next", "dist", "build"}
DEFAULT_ROOT = Path(__file__).resolve().parent.parent


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Enforce code file line-count limit with a small whitelist."
    )
    parser.add_argument(
        "--root",
        default=str(DEFAULT_ROOT),
        help="Repository root to scan",
    )
    parser.add_argument(
        "--limit", type=int, default=DEFAULT_LIMIT, help="Maximum allowed line count"
    )
    parser.add_argument(
        "--whitelist",
        default="scripts/line-limit-whitelist.txt",
        help="Relative path to whitelist file",
    )
    return parser.parse_args()


def load_whitelist(root: Path, whitelist_path: str) -> set[str]:
    path = root / whitelist_path
    if not path.exists():
        return set()

    entries: set[str] = set()
    for raw in path.read_text().splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        entries.add(line)
    return entries


def should_skip(path: Path) -> bool:
    return any(part in DEFAULT_EXCLUDE_DIRS for part in path.parts)


def is_code_file(path: Path) -> bool:
    return path.suffix in DEFAULT_EXTENSIONS


def count_lines(path: Path) -> int:
    with path.open("r", encoding="utf-8", errors="ignore") as handle:
        return sum(1 for _ in handle)


def main() -> int:
    args = parse_args()
    root = Path(args.root).resolve()
    whitelist = load_whitelist(root, args.whitelist)
    violations: list[tuple[int, str]] = []

    for path in sorted(root.rglob("*")):
        if not path.is_file() or should_skip(path) or not is_code_file(path):
            continue

        rel = path.relative_to(root).as_posix()
        if rel in whitelist:
            continue

        lines = count_lines(path)
        if lines > args.limit:
            violations.append((lines, rel))

    if violations:
        print(
            f"code line-limit violations found (limit={args.limit}, whitelist={args.whitelist}):",
            file=sys.stderr,
        )
        for lines, rel in violations:
            print(f"  {lines:5d} {rel}", file=sys.stderr)
        return 1

    print(
        f"code line-limit ok (limit={args.limit}, whitelist_entries={len(whitelist)})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
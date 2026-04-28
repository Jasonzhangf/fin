#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path
from typing import Any


LEGACY_TEST_SESSION_EXACT = {
    "session-cli-demo",
}

TEST_SESSION_PREFIXES = (
    "session-test-",
    "session-mainline-demo",
    "session-transcript-demo",
)

TEST_RUN_PREFIXES = (
    "test-",
    "qqbot-live-receipt-",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Remove disposable test sessions / test harness run leftovers and scrub "
            "main runtime pointers that still reference removed test sessions."
        )
    )
    parser.add_argument(
        "--runtime-home",
        default=str(Path.home() / ".fin"),
        help="Target fin runtime home. Defaults to ~/.fin",
    )
    parser.add_argument(
        "--keep-harness-runs",
        action="store_true",
        help="Keep harness/runs/* receipts; only delete session leftovers and scrub pointers.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print what would be removed / scrubbed without changing files.",
    )
    return parser.parse_args()


def is_disposable_test_session(session_id: str) -> bool:
    return session_id in LEGACY_TEST_SESSION_EXACT or session_id.startswith(TEST_SESSION_PREFIXES)


def is_isolated_test_runtime_home(runtime_home: Path) -> bool:
    if runtime_home.name != "runtime-home":
        return False
    run_dir = runtime_home.parent
    return run_dir.is_dir() and run_dir.name.startswith(TEST_RUN_PREFIXES)


def load_json(path: Path) -> Any | None:
    if not path.exists():
        return None
    return json.loads(path.read_text())


def write_json(path: Path, value: Any, dry_run: bool) -> None:
    if dry_run:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def remove_path(path: Path, dry_run: bool) -> None:
    if dry_run:
        return
    if path.is_dir():
        shutil.rmtree(path, ignore_errors=True)
    elif path.exists():
        path.unlink()


def find_test_sessions(runtime_home: Path) -> list[Path]:
    root = runtime_home / "sessions"
    found: list[Path] = []
    if not root.exists():
        return found
    isolated_test_home = is_isolated_test_runtime_home(runtime_home)
    for year_dir in sorted(root.iterdir()):
        if not year_dir.is_dir():
            continue
        for month_dir in sorted(year_dir.iterdir()):
            if not month_dir.is_dir():
                continue
            for session_dir in sorted(month_dir.iterdir()):
                if not session_dir.is_dir():
                    continue
                if isolated_test_home or is_disposable_test_session(session_dir.name):
                    found.append(session_dir)
    return found


def find_test_runs(runtime_home: Path) -> list[Path]:
    root = runtime_home / "harness" / "runs"
    if not root.exists():
        return []
    return [
        run_dir
        for run_dir in sorted(root.iterdir())
        if run_dir.is_dir() and run_dir.name.startswith(TEST_RUN_PREFIXES)
    ]


def scrub_last_run(runtime_home: Path, removed_session_ids: set[str], dry_run: bool) -> list[str]:
    path = runtime_home / "runtime" / "current" / "last_run.json"
    value = load_json(path)
    if not isinstance(value, dict):
        return []
    if value.get("session_id") not in removed_session_ids:
        return []
    write_json(path, {}, dry_run)
    return [str(path)]


def scrub_qqbot_conversations(
    runtime_home: Path, removed_session_ids: set[str], dry_run: bool
) -> list[str]:
    path = runtime_home / "runtime" / "channels" / "qqbot" / "conversations.json"
    value = load_json(path)
    if not isinstance(value, dict):
        return []
    conversations = value.get("conversations")
    if not isinstance(conversations, list):
        return []
    filtered = [
        item
        for item in conversations
        if not (
            isinstance(item, dict) and item.get("session_id") in removed_session_ids
        )
    ]
    if len(filtered) == len(conversations):
        return []
    value["conversations"] = filtered
    write_json(path, value, dry_run)
    return [str(path)]


def scrub_qqbot_state(runtime_home: Path, removed_session_ids: set[str], dry_run: bool) -> list[str]:
    path = runtime_home / "runtime" / "peers" / "qqbot" / "state.json"
    value = load_json(path)
    if not isinstance(value, dict):
        return []
    if value.get("session_id") not in removed_session_ids:
        return []
    value["binding_state"] = "unbound"
    value["pairing_required"] = False
    value["session_valid"] = False
    value["paired_at"] = None
    value["session_id"] = None
    value["session_expires_at"] = None
    value["session_ttl_minutes"] = None
    write_json(path, value, dry_run)
    return [str(path)]


def scrub_activity_delivery_state(
    runtime_home: Path, removed_session_ids: set[str], dry_run: bool
) -> list[str]:
    path = runtime_home / "runtime" / "peers" / "qqbot" / "activity_delivery_state.json"
    value = load_json(path)
    if not isinstance(value, dict):
        return []
    if value.get("session_id") not in removed_session_ids:
        return []
    value["session_id"] = None
    value["target"] = None
    value["last_user_signature"] = None
    value["last_delivered_text"] = None
    value["last_delivery_reason"] = None
    value["last_delivery_at"] = None
    write_json(path, value, dry_run)
    return [str(path)]


def main() -> int:
    args = parse_args()
    runtime_home = Path(args.runtime_home).expanduser().resolve()

    test_session_dirs = find_test_sessions(runtime_home)
    removed_session_ids = {path.name for path in test_session_dirs}
    test_run_dirs = [] if args.keep_harness_runs else find_test_runs(runtime_home)

    scrubbed_files: list[str] = []
    if removed_session_ids:
        scrubbed_files.extend(scrub_last_run(runtime_home, removed_session_ids, args.dry_run))
        scrubbed_files.extend(
            scrub_qqbot_conversations(runtime_home, removed_session_ids, args.dry_run)
        )
        scrubbed_files.extend(scrub_qqbot_state(runtime_home, removed_session_ids, args.dry_run))
        scrubbed_files.extend(
            scrub_activity_delivery_state(runtime_home, removed_session_ids, args.dry_run)
        )

    for session_dir in test_session_dirs:
        remove_path(session_dir, args.dry_run)
    for run_dir in test_run_dirs:
        remove_path(run_dir, args.dry_run)

    mode = "DRY-RUN" if args.dry_run else "APPLIED"
    print(f"[cleanup-test-sessions] {mode}")
    print(f"runtime_home={runtime_home}")
    print("removed_sessions=")
    for path in test_session_dirs:
        print(f"  - {path}")
    print("removed_harness_runs=")
    for path in test_run_dirs:
        print(f"  - {path}")
    print("scrubbed_files=")
    for path in scrubbed_files:
        print(f"  - {path}")
    print(
        "summary="
        f"sessions={len(test_session_dirs)} "
        f"runs={len(test_run_dirs)} "
        f"scrubbed={len(scrubbed_files)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

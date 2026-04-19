#!/usr/bin/env python3
import argparse
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "fin.receipt.index.v1"


def read_json(path: Path) -> dict[str, Any] | None:
    if not path.exists():
        return None
    return json.loads(path.read_text())


def rel(path: Path, base: Path) -> str:
    try:
        return str(path.relative_to(base))
    except ValueError:
        return str(path)


def build_install_smoke(report_dir: Path, runtime_home: Path, build_version: str) -> dict[str, Any]:
    summary_path = report_dir / "summary.json"
    summary = read_json(summary_path)
    install_receipt_path = runtime_home / "install" / "receipts" / f"{build_version}.json"
    install_state_path = runtime_home / "runtime" / "current" / "install_state.json"
    install_receipt = read_json(install_receipt_path)
    install_state = read_json(install_state_path)
    if summary is None and install_receipt is None:
        return missing_receipt("install_smoke")
    verified_paths = summary.get("verified_paths", []) if summary else []
    return {
        "receipt_id": "install_smoke",
        "kind": "install_smoke",
        "status": "passed",
        "report_path": rel(summary_path, runtime_home) if summary else None,
        "supporting_paths": [
            p
            for p in [
                rel(install_receipt_path, runtime_home) if install_receipt else None,
                rel(install_state_path, runtime_home) if install_state else None,
            ]
            if p
        ],
        "summary": {
            "build_version": build_version,
            "binary": summary.get("binary") if summary else None,
            "session_id": summary.get("session_id") if summary else None,
            "task_id": summary.get("task_id") if summary else None,
            "operation_id": summary.get("operation_id") if summary else None,
            "verified_paths_count": len(verified_paths),
            "verified_paths": verified_paths,
            "install_action": install_receipt.get("action") if install_receipt else None,
            "current_version": install_receipt.get("current_version") if install_receipt else None,
            "git_sha": install_receipt.get("git_sha") if install_receipt else None,
            "install_state_build_version": install_state.get("build_version") if install_state else None,
        },
    }


def build_provider_probe(report_dir: Path, runtime_home: Path) -> dict[str, Any]:
    path = report_dir / "provider-probe.json"
    data = read_json(path)
    if data is None:
        return missing_receipt("provider_probe")
    return {
        "receipt_id": "provider_probe",
        "kind": "provider_probe",
        "status": "passed" if data.get("ok") else "failed",
        "report_path": rel(path, runtime_home),
        "supporting_paths": [],
        "summary": {
            "provider": data.get("provider"),
            "protocol": data.get("protocol"),
            "model": data.get("model"),
            "endpoint": data.get("endpoint"),
            "status": data.get("status"),
            "stop_reason": data.get("stop_reason"),
            "user_agent": data.get("user_agent"),
        },
    }


def build_compact_rebuild(report_dir: Path, runtime_home: Path) -> dict[str, Any]:
    response_path = report_dir / "web-debug-compact-response.json"
    handcheck_path = report_dir / "web-debug-handcheck.json"
    data = read_json(response_path)
    if data is None:
        return missing_receipt("compact_rebuild")
    binding = data.get("binding", {})
    return {
        "receipt_id": "compact_rebuild",
        "kind": "compact_rebuild",
        "status": "passed",
        "report_path": rel(response_path, runtime_home),
        "supporting_paths": [
            rel(handcheck_path, runtime_home)
            for p in [handcheck_path]
            if p.exists()
        ],
        "summary": {
            "response_kind": data.get("response_kind"),
            "events_count": data.get("events_count"),
            "session_id": binding.get("session_id"),
            "task_id": binding.get("task_id"),
            "digest_id": data.get("digest_id"),
            "answer": data.get("answer"),
        },
    }


def build_status_probe(report_dir: Path, runtime_home: Path) -> dict[str, Any]:
    response_path = report_dir / "web-debug-status-response.json"
    binding_path = report_dir / "web-debug-binding.json"
    handcheck_path = report_dir / "web-debug-handcheck.json"
    data = read_json(response_path)
    if data is None:
        return missing_receipt("status_probe")
    binding = data.get("binding", {})
    return {
        "receipt_id": "status_probe",
        "kind": "status_probe",
        "status": "passed" if data.get("response_kind") == "status_probe" else "failed",
        "report_path": rel(response_path, runtime_home),
        "supporting_paths": [
            rel(p, runtime_home)
            for p in [binding_path, handcheck_path]
            if p.exists()
        ],
        "summary": {
            "response_kind": data.get("response_kind"),
            "freshness": data.get("freshness"),
            "events_count": data.get("events_count"),
            "session_id": binding.get("session_id"),
            "task_id": binding.get("task_id"),
            "digest_id": data.get("digest_id"),
        },
    }


def build_installed_binary_manual(report_dir: Path, runtime_home: Path) -> dict[str, Any]:
    path = report_dir / "installed-binary-smoke-manual.json"
    data = read_json(path)
    if data is None:
        return missing_receipt("installed_binary_smoke_manual")
    return {
        "receipt_id": "installed_binary_smoke_manual",
        "kind": "installed_binary_smoke_manual",
        "status": "passed" if data.get("ok") else "failed",
        "report_path": rel(path, runtime_home),
        "supporting_paths": [
            rel(Path(data["regression_log"]), runtime_home)
            if data.get("regression_log")
            else None
        ],
        "summary": {
            "run_id": data.get("run_id"),
            "session_id": data.get("session_id"),
            "task_id": data.get("task_id"),
            "operation_id": data.get("operation_id"),
            "verified_paths_count": len((data.get("verified_paths") or {}).keys()),
        },
    }


def missing_receipt(kind: str) -> dict[str, Any]:
    return {
        "receipt_id": kind,
        "kind": kind,
        "status": "missing",
        "report_path": None,
        "supporting_paths": [],
        "summary": {},
    }


def normalize_paths(receipts: list[dict[str, Any]]) -> None:
    for receipt in receipts:
        receipt["supporting_paths"] = [p for p in receipt.get("supporting_paths", []) if p]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report-dir", required=True)
    parser.add_argument("--runtime-home", required=True)
    parser.add_argument("--build-version", required=True)
    parser.add_argument("--output")
    args = parser.parse_args()

    report_dir = Path(args.report_dir).expanduser().resolve()
    runtime_home = Path(args.runtime_home).expanduser().resolve()
    output = Path(args.output).expanduser().resolve() if args.output else report_dir / "receipt-index.json"

    receipts = [
        build_install_smoke(report_dir, runtime_home, args.build_version),
        build_provider_probe(report_dir, runtime_home),
        build_compact_rebuild(report_dir, runtime_home),
        build_status_probe(report_dir, runtime_home),
        build_installed_binary_manual(report_dir, runtime_home),
    ]
    normalize_paths(receipts)

    payload = {
        "schema_version": SCHEMA_VERSION,
        "generated_at": datetime.now(timezone.utc).astimezone().isoformat(timespec="seconds"),
        "build_version": args.build_version,
        "runtime_home": str(runtime_home),
        "report_dir": str(report_dir),
        "receipts": receipts,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n")
    print(output)


if __name__ == "__main__":
    main()

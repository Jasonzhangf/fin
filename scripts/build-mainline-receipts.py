#!/usr/bin/env python3
import argparse
import json
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "fin.receipt.mainline.v1"


def read_json(path: Path) -> Any | None:
    if not path.exists():
        return None
    return json.loads(path.read_text())


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    rows: list[dict[str, Any]] = []
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line:
            continue
        rows.append(json.loads(line))
    return rows


def rel(path: Path, base: Path) -> str:
    try:
        return str(path.relative_to(base))
    except ValueError:
        return str(path)


def session_path(runtime_home: Path, session_id: str) -> Path:
    matches = sorted(runtime_home.glob(f"sessions/*/*/{session_id}"))
    if not matches:
        raise FileNotFoundError(f"session not found: {session_id}")
    if len(matches) > 1:
        raise RuntimeError(f"session is ambiguous: {session_id}")
    return matches[0]


def missing_receipt(kind: str, session_id: str, session_dir: Path, runtime_home: Path, reason: str) -> dict[str, Any]:
    return {
        "receipt_id": kind,
        "kind": kind,
        "status": "missing",
        "source_session_id": session_id,
        "source_session_path": rel(session_dir, runtime_home),
        "supporting_paths": [],
        "summary": {"reason": reason},
    }


def build_history_context(session_id: str, session_dir: Path, runtime_home: Path) -> dict[str, Any]:
    messages_path = session_dir / "conversation" / "messages.json"
    contexts_path = session_dir / "context" / "recent_contexts.json"
    steps_path = session_dir / "steps" / "recent_steps.json"
    rounds_path = session_dir / "rounds" / "recent_rounds.json"

    messages = read_json(messages_path) or []
    contexts = read_json(contexts_path) or []
    steps = read_json(steps_path) or []
    rounds = read_json(rounds_path) or []
    round_operation_ids = {item.get("operation_id") for item in rounds if item.get("operation_id")}
    filtered_contexts = [
        item for item in contexts if item.get("operation_id") in round_operation_ids
    ] or contexts
    if len(filtered_contexts) < 2:
        return missing_receipt(
            "history_context",
            session_id,
            session_dir,
            runtime_home,
            "need at least 2 round-backed context records",
        )

    growth = []
    for item in filtered_contexts:
        continuity_tail = item.get("context", {}).get("continuity_tail") or []
        growth.append(
            {
                "operation_id": item.get("operation_id"),
                "continuity_tail_messages": len(continuity_tail),
                "tail_preview": continuity_tail[-4:],
                "input": item.get("input"),
            }
        )
    first_tail = growth[0]["continuity_tail_messages"]
    max_tail = max(item["continuity_tail_messages"] for item in growth)
    latest = growth[-1]
    passed = first_tail == 0 and max_tail > 0 and len(rounds) >= 2

    step_summaries = [
        {
            "operation_id": step.get("operation_id"),
            "step_id": step.get("step_id"),
            "step_index": step.get("step_index"),
            "summary": step.get("summary"),
        }
        for step in steps
        if step.get("step_kind") == "context_build"
    ]
    counts = Counter(message.get("role") for message in messages)
    return {
        "receipt_id": "history_context",
        "kind": "history_context",
        "status": "passed" if passed else "failed",
        "source_session_id": session_id,
        "source_session_path": rel(session_dir, runtime_home),
        "supporting_paths": [
            rel(path, runtime_home)
            for path in [messages_path, contexts_path, steps_path, rounds_path]
            if path.exists()
        ],
        "summary": {
            "operation_count": len({item.get("operation_id") for item in filtered_contexts}),
            "round_count": len(rounds),
            "message_count": len(messages),
            "user_message_count": counts.get("user", 0),
            "assistant_message_count": counts.get("assistant", 0),
            "first_continuity_tail_messages": first_tail,
            "max_continuity_tail_messages": max_tail,
            "latest_continuity_tail_messages": latest["continuity_tail_messages"],
            "latest_tail_preview": latest["tail_preview"],
            "latest_input": latest["input"],
            "ignored_auxiliary_context_records": max(len(contexts) - len(filtered_contexts), 0),
            "continuity_growth": growth,
            "context_build_steps": step_summaries,
        },
    }


def build_auto_tool_roundtrip(session_id: str, session_dir: Path, runtime_home: Path) -> dict[str, Any]:
    rounds_path = session_dir / "rounds" / "recent_rounds.json"
    steps_path = session_dir / "steps" / "recent_steps.json"
    events_path = session_dir / "events" / "stream.jsonl"
    tools_path = session_dir / "tools" / "recent_tool_records.json"

    rounds = read_json(rounds_path) or []
    steps = read_json(steps_path) or []
    events = read_jsonl(events_path)
    tools = read_json(tools_path) or []

    rounds_by_operation: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for round_record in rounds:
        rounds_by_operation[round_record.get("operation_id")].append(round_record)
    multi_round_ops = {
        op_id: sorted(records, key=lambda item: item.get("round_index") or 0)
        for op_id, records in rounds_by_operation.items()
        if len(records) > 1
    }
    roundtrip_events = [
        event
        for event in events
        if event.get("event_type") == "reasoning.auto_tool_roundtrip_completed"
    ]
    if not multi_round_ops and not roundtrip_events:
        return missing_receipt(
            "auto_tool_roundtrip",
            session_id,
            session_dir,
            runtime_home,
            "no multi-round tool-loop evidence in current session artifacts",
        )

    tool_dispatch_steps = [
        {
            "operation_id": step.get("operation_id"),
            "step_id": step.get("step_id"),
            "step_index": step.get("step_index"),
            "summary": step.get("summary"),
        }
        for step in steps
        if step.get("step_kind") == "tool_dispatch"
    ]
    return {
        "receipt_id": "auto_tool_roundtrip",
        "kind": "auto_tool_roundtrip",
        "status": "passed",
        "source_session_id": session_id,
        "source_session_path": rel(session_dir, runtime_home),
        "supporting_paths": [
            rel(path, runtime_home)
            for path in [rounds_path, steps_path, events_path, tools_path]
            if path.exists()
        ],
        "summary": {
            "multi_round_operation_ids": sorted(multi_round_ops.keys()),
            "max_rounds_in_operation": max((len(records) for records in multi_round_ops.values()), default=0),
            "roundtrip_event_count": len(roundtrip_events),
            "tool_record_count": len(tools),
            "tool_dispatch_steps": tool_dispatch_steps,
        },
    }


def build_control_boundary(session_id: str, session_dir: Path, runtime_home: Path) -> dict[str, Any]:
    candidate_paths = {
        "supervisor_latest_heartbeat": session_dir / "control" / "supervisor" / "latest_heartbeat.json",
        "supervisor_recent_heartbeats": session_dir / "control" / "supervisor" / "recent_heartbeats.json",
        "daemon_latest_state": session_dir / "control" / "daemon" / "latest_state.json",
        "daemon_latest_recovery_action": session_dir / "control" / "daemon" / "latest_recovery_action.json",
        "queue_pending_inputs": session_dir / "queue" / "pending_inputs.json",
        "interrupts_recent_segments": session_dir / "interrupts" / "recent_segments.json",
        "interrupts_recent_merges": session_dir / "interrupts" / "recent_merges.json",
        "runtime_current_execution_state": runtime_home / "runtime" / "current" / "current_execution_state.json",
    }
    present = {name: path for name, path in candidate_paths.items() if path.exists()}
    if not present:
        return missing_receipt(
            "control_boundary",
            session_id,
            session_dir,
            runtime_home,
            "no durable control-plane artifacts found",
        )

    summary: dict[str, Any] = {
        "present_families": sorted(present.keys()),
        "queue_pending_inputs": 0,
        "interrupt_open_segments": 0,
        "interrupt_merge_records": 0,
        "heartbeat_status": None,
        "daemon_state": None,
        "daemon_recovery_action": None,
        "execution_phase": None,
    }
    pending = read_json(candidate_paths["queue_pending_inputs"]) or []
    segments = read_json(candidate_paths["interrupts_recent_segments"]) or []
    merges = read_json(candidate_paths["interrupts_recent_merges"]) or []
    heartbeat = read_json(candidate_paths["supervisor_latest_heartbeat"]) or {}
    daemon_state = read_json(candidate_paths["daemon_latest_state"]) or {}
    recovery = read_json(candidate_paths["daemon_latest_recovery_action"]) or {}
    execution_state = read_json(candidate_paths["runtime_current_execution_state"]) or {}

    summary["queue_pending_inputs"] = len(pending) if isinstance(pending, list) else 0
    summary["interrupt_open_segments"] = len(segments) if isinstance(segments, list) else 0
    summary["interrupt_merge_records"] = len(merges) if isinstance(merges, list) else 0
    summary["heartbeat_status"] = heartbeat.get("status")
    summary["daemon_state"] = daemon_state.get("lifecycle_state")
    summary["daemon_recovery_action"] = recovery.get("action_kind")
    summary["execution_phase"] = execution_state.get("phase")

    return {
        "receipt_id": "control_boundary",
        "kind": "control_boundary",
        "status": "passed",
        "source_session_id": session_id,
        "source_session_path": rel(session_dir, runtime_home),
        "supporting_paths": [rel(path, runtime_home) for path in present.values()],
        "summary": summary,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runtime-home", required=True)
    parser.add_argument("--report-dir", required=True)
    parser.add_argument("--build-version", required=True)
    parser.add_argument("--session-id", required=True)
    parser.add_argument("--output")
    args = parser.parse_args()

    runtime_home = Path(args.runtime_home).expanduser().resolve()
    report_dir = Path(args.report_dir).expanduser().resolve()
    output = Path(args.output).expanduser().resolve() if args.output else report_dir / "mainline-receipts.json"
    session_dir = session_path(runtime_home, args.session_id)

    receipts = [
        build_history_context(args.session_id, session_dir, runtime_home),
        build_auto_tool_roundtrip(args.session_id, session_dir, runtime_home),
        build_control_boundary(args.session_id, session_dir, runtime_home),
    ]
    payload = {
        "schema_version": SCHEMA_VERSION,
        "generated_at": datetime.now(timezone.utc).astimezone().isoformat(timespec="seconds"),
        "build_version": args.build_version,
        "runtime_home": str(runtime_home),
        "report_dir": str(report_dir),
        "source_session_id": args.session_id,
        "source_session_path": rel(session_dir, runtime_home),
        "receipts": receipts,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n")
    print(output)


if __name__ == "__main__":
    main()

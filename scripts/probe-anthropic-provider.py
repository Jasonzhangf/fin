#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import sys
import tomllib
import urllib.error
import urllib.request


ANTHROPIC_VERSION = "2023-06-01"
DEFAULT_USER_AGENT = "fin-coding-agent/0.1"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run a real anthropic-wire provider probe from fin user.toml."
    )
    parser.add_argument("--user-toml", required=True, help="Path to fin user.toml")
    parser.add_argument(
        "--report",
        required=True,
        help="Where to write the probe report json",
    )
    parser.add_argument(
        "--expected-model",
        default="qwen3.6-plus",
        help="Expected default model",
    )
    return parser.parse_args()


def load_user_toml(path: Path) -> dict:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def resolve_api_key(provider: dict) -> tuple[str, str]:
    direct = provider.get("api_key")
    env_name = provider.get("api_key_env")
    if direct and env_name:
        raise SystemExit("provider config cannot set both api_key and api_key_env")
    if direct:
        return ("direct", direct)
    if env_name:
        env_value = os.environ.get(env_name)
        if not env_value:
            raise SystemExit(f"api_key_env '{env_name}' is not set or empty")
        return ("env", env_value)
    raise SystemExit("provider config requires api_key or api_key_env")


def build_payload(model: str) -> bytes:
    payload = {
        "model": model,
        "max_tokens": 16,
        "messages": [
            {
                "role": "user",
                "content": "Reply with exactly OK.",
            }
        ],
    }
    return json.dumps(payload).encode("utf-8")


def main() -> int:
    args = parse_args()
    user_toml = Path(args.user_toml).expanduser()
    report_path = Path(args.report).expanduser()
    report_path.parent.mkdir(parents=True, exist_ok=True)

    user = load_user_toml(user_toml)
    provider_name = user["default_provider"]
    provider = user["providers"][provider_name]

    protocol = provider["protocol"]
    base_url = provider["base_url"].rstrip("/")
    model = provider["model"]
    key_source, api_key = resolve_api_key(provider)
    custom_headers = provider.get("headers", {})
    user_agent = provider.get("user_agent") or DEFAULT_USER_AGENT

    if protocol != "anthropic-wire":
        raise SystemExit(f"default provider protocol must be anthropic-wire, got {protocol}")
    if model != args.expected_model:
        raise SystemExit(f"default provider model must be {args.expected_model}, got {model}")

    endpoint = f"{base_url}/v1/messages"
    payload = build_payload(model)
    headers = dict(custom_headers)
    headers["content-type"] = "application/json"
    headers["accept"] = "application/json"
    headers["x-api-key"] = api_key
    headers["anthropic-version"] = ANTHROPIC_VERSION
    headers["user-agent"] = user_agent

    request = urllib.request.Request(endpoint, data=payload, headers=headers, method="POST")

    report: dict[str, object] = {
        "provider": provider_name,
        "protocol": protocol,
        "model": model,
        "endpoint": endpoint,
        "anthropic_version": ANTHROPIC_VERSION,
        "api_key_source": key_source,
        "user_agent": user_agent,
        "custom_header_names": sorted(custom_headers.keys()),
    }

    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            body = response.read()
            parsed = json.loads(body)
            text_parts = [
                item.get("text", "")
                for item in parsed.get("content", [])
                if isinstance(item, dict) and item.get("type") == "text"
            ]
            text = "".join(text_parts).strip()
            report.update(
                {
                    "status": response.status,
                    "request_id": response.headers.get("request-id"),
                    "response_id": parsed.get("id"),
                    "stop_reason": parsed.get("stop_reason"),
                    "output_text": text,
                }
            )
            ok = response.status == 200 and "OK" in text
            report["ok"] = ok
            report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2))
            print(json.dumps(report, ensure_ascii=False, indent=2))
            return 0 if ok else 1
    except urllib.error.HTTPError as err:
        body = err.read().decode("utf-8", errors="replace")
        report.update({"status": err.code, "ok": False, "error_body": body})
        report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2))
        print(json.dumps(report, ensure_ascii=False, indent=2), file=sys.stderr)
        return 1
    except Exception as err:  # noqa: BLE001
        report.update({"ok": False, "error": str(err)})
        report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2))
        print(json.dumps(report, ensure_ascii=False, indent=2), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
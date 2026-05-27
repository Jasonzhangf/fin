#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import sys
import urllib.error
import urllib.request

try:
    import tomllib  # type: ignore
    HAS_TOMLLIB = True
except ModuleNotFoundError:
    HAS_TOMLLIB = False
    try:
        import toml  # type: ignore
    except ModuleNotFoundError as exc:  # pragma: no cover - env-dependent
        raise SystemExit("python tomllib or toml is required to parse user.toml") from exc


DEFAULT_USER_AGENT = "fin-coding-agent/0.1"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run a real provider probe from fin user.toml."
    )
    parser.add_argument("--user-toml", required=True, help="Path to fin user.toml")
    parser.add_argument(
        "--report",
        required=True,
        help="Where to write the probe report json",
    )
    parser.add_argument(
        "--expected-model",
        default="MiniMax-M2.7",
        help="Expected default model",
    )
    return parser.parse_args()


def load_user_toml(path: Path) -> dict:
    raw = path.read_text()
    if HAS_TOMLLIB:
        return tomllib.loads(raw)  # type: ignore[name-defined]
    return toml.loads(raw)  # type: ignore[name-defined]


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


def build_payload(protocol: str, model: str) -> bytes:
    if protocol == "anthropic-wire":
        payload = {
            "model": model,
            "max_tokens": 128,
            "messages": [
                {
                    "role": "user",
                    "content": "Reply with exactly OK.",
                }
            ],
        }
    elif protocol == "open-ai-compatible":
        payload = {
            "model": model,
            "messages": [
                {
                    "role": "user",
                    "content": "Reply with exactly OK.",
                }
            ],
            "max_tokens": 128,
            "temperature": 0,
        }
    else:
        raise SystemExit(f"unsupported protocol '{protocol}'")
    return json.dumps(payload).encode("utf-8")


def build_request(protocol: str, base_url: str, api_key: str, user_agent: str, custom_headers: dict, payload: bytes) -> urllib.request.Request:
    headers = dict(custom_headers)
    headers["content-type"] = "application/json"
    headers["accept"] = "application/json"
    headers["user-agent"] = user_agent
    if protocol == "anthropic-wire":
        endpoint = f"{base_url}/v1/messages"
        headers["x-api-key"] = api_key
        headers["anthropic-version"] = "2023-06-01"
    elif protocol == "open-ai-compatible":
        endpoint = f"{base_url}/v1/chat/completions"
        headers["authorization"] = f"Bearer {api_key}"
    else:
        raise SystemExit(f"unsupported protocol '{protocol}'")
    return urllib.request.Request(endpoint, data=payload, headers=headers, method="POST")


def extract_text(protocol: str, parsed: dict) -> str:
    if protocol == "anthropic-wire":
        text_parts = [
            item.get("text", "")
            for item in parsed.get("content", [])
            if isinstance(item, dict) and item.get("type") == "text"
        ]
        return "".join(text_parts).strip()
    if protocol == "open-ai-compatible":
        choices = parsed.get("choices", [])
        if not isinstance(choices, list):
            return ""
        parts: list[str] = []
        for choice in choices:
            if not isinstance(choice, dict):
                continue
            message = choice.get("message")
            if isinstance(message, dict):
                content = message.get("content", "")
                if isinstance(content, str):
                    parts.append(content)
        return "\n".join(part.strip() for part in parts if part.strip()).strip()
    return ""


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

    if model != args.expected_model:
        raise SystemExit(f"default provider model must be {args.expected_model}, got {model}")

    payload = build_payload(protocol, model)
    request = build_request(protocol, base_url, api_key, user_agent, custom_headers, payload)
    endpoint = request.full_url

    report: dict[str, object] = {
        "provider": provider_name,
        "protocol": protocol,
        "model": model,
        "endpoint": endpoint,
        "api_key_source": key_source,
        "user_agent": user_agent,
        "custom_header_names": sorted(custom_headers.keys()),
    }
    if protocol == "anthropic-wire":
        report["anthropic_version"] = "2023-06-01"

    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            body = response.read()
            parsed = json.loads(body)
            text = extract_text(protocol, parsed)
            reasoning_text = ""
            if protocol == "open-ai-compatible":
                choices = parsed.get("choices", [])
                if isinstance(choices, list) and choices:
                    message = choices[0].get("message", {})
                    if isinstance(message, dict):
                        reasoning_text = str(message.get("reasoning_content", "") or "").strip()
            report.update(
                {
                    "status": response.status,
                    "request_id": response.headers.get("request-id"),
                    "response_id": parsed.get("id"),
                    "stop_reason": parsed.get("stop_reason")
                    or (
                        parsed.get("choices", [{}])[0].get("finish_reason")
                        if isinstance(parsed.get("choices"), list) and parsed.get("choices")
                        else None
                    ),
                    "output_text": text,
                    "reasoning_text": reasoning_text,
                }
            )
            has_business_error = False
            if isinstance(parsed.get("base_resp"), dict):
                status_code = parsed["base_resp"].get("status_code", 0)
                has_business_error = bool(status_code)
                report["base_resp_status_code"] = status_code
                report["base_resp_status_msg"] = parsed["base_resp"].get("status_msg", "")
            ok = response.status == 200 and not has_business_error and bool(text or reasoning_text)
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

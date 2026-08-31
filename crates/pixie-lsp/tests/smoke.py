#!/usr/bin/env python3
"""Smoke test: spawn cute-lsp, send initialize + didOpen of a buggy
buffer, and assert that the server replies to initialize and publishes
at least one diagnostic. Run via `python3 crates/cute-lsp/tests/smoke.py`
from the workspace root after `cargo build -p cute-lsp`."""

import json
import subprocess
import sys
import time

BIN = "target/debug/cute-lsp"


def frame(payload: dict) -> bytes:
    body = json.dumps(payload).encode("utf-8")
    return f"Content-Length: {len(body)}\r\n\r\n".encode("ascii") + body


def read_one(stream) -> dict:
    """Read one Content-Length-framed JSON message from `stream`."""
    headers = {}
    while True:
        line = stream.readline()
        if not line:
            raise RuntimeError("server closed stdout before sending a message")
        line = line.decode("ascii").rstrip("\r\n")
        if line == "":
            break
        if ":" in line:
            k, v = line.split(":", 1)
            headers[k.strip().lower()] = v.strip()
    n = int(headers["content-length"])
    body = stream.read(n)
    return json.loads(body)


def main() -> int:
    proc = subprocess.Popen(
        [BIN],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert proc.stdin and proc.stdout

    proc.stdin.write(frame({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {"capabilities": {}},
    }))
    proc.stdin.flush()

    init_resp = read_one(proc.stdout)
    assert init_resp.get("id") == 1, init_resp
    caps = init_resp["result"]["capabilities"]
    assert "textDocumentSync" in caps, caps
    print("[ok] initialize ->", init_resp["result"]["serverInfo"])

    proc.stdin.write(frame({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": {},
    }))
    proc.stdin.write(frame({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///tmp/smoke.cute",
                "languageId": "cute",
                "version": 1,
                "text": "fn main { ",
            }
        },
    }))
    proc.stdin.flush()

    deadline = time.time() + 5.0
    diags = None
    while time.time() < deadline:
        msg = read_one(proc.stdout)
        if msg.get("method") == "window/logMessage":
            continue
        if msg.get("method") == "textDocument/publishDiagnostics":
            diags = msg["params"]["diagnostics"]
            break
    assert diags is not None, "no publishDiagnostics within timeout"
    assert len(diags) >= 1, f"expected >= 1 diagnostic, got {diags}"
    print(f"[ok] publishDiagnostics: {len(diags)} entry — {diags[0]['message'][:60]}")

    proc.stdin.write(frame({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "shutdown",
        "params": None,
    }))
    proc.stdin.flush()
    read_one(proc.stdout)

    proc.stdin.write(frame({
        "jsonrpc": "2.0",
        "method": "exit",
        "params": None,
    }))
    proc.stdin.flush()
    proc.stdin.close()  # Signal EOF so the server's stdin reader unblocks.
    try:
        proc.wait(timeout=3)
    except subprocess.TimeoutExpired:
        proc.kill()
        raise
    print("[ok] shutdown + exit")
    return 0


if __name__ == "__main__":
    sys.exit(main())

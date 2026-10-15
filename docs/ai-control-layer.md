# KairOS AI Control Layer

The AI control layer connects KairOS to an external inference server over TCP. It runs as a normal hosted service (`std` build) — not inside privileged kernel execution.

## Architecture

```text
terminal → ai_cmd → ai bridge → TCP → external AI server
```

KairOS creates and tracks every AI job, assigns process and request IDs, records phase transitions in the statechain, and authorizes tool requests at `Phi3`.

**Management network only (v1):** traffic is plain TCP with newline-delimited JSON. Do not expose the bridge on a public network.

## Quick start

### Mock server (tests / local demo)

```text
kairos> ai connect mock 8000
AI bridge connected to mock:8000

kairos> ai ask summarize current process phases
AI job 1 created
PID: 2
Phase: Phi0
Status: completed

kairos> ai show 1
kairos> ai trace 1
```

### External AI server

Run any server that accepts one JSON request per line and returns one JSON response per line.

**Request fields:** `request_id`, `process_id`, `model`, `input`, `input_hash`, `allowed_tools`, `deadline`, `current_phase`, `parent_chain_hash`

**Response fields:** `request_id`, `status`, `output`, `output_hash`, `runtime`, `error`, `tool_requests`

Example minimal Python server:

```python
import json, socket

def handle(conn):
    line = conn.recv(8192).decode().strip()
    req = json.loads(line)
    resp = {
        "request_id": req["request_id"],
        "status": "ok",
        "output": "Kernel state is stable.",
        "output_hash": "91ac",
        "runtime": 3,
        "error": "",
        "tool_requests": [],
    }
    conn.sendall((json.dumps(resp) + "\n").encode())

s = socket.socket()
s.bind(("0.0.0.0", 8000))
s.listen(1)
while True:
    c, _ = s.accept()
    handle(c)
    c.close()
```

Connect from KairOS:

```text
kairos> ai connect 10.0.0.20 8000
AI bridge connected to 10.0.0.20:8000
```

## Commands

| Command | Description |
|---------|-------------|
| `ai connect <host> <port>` | Open TCP connection (use `mock` for in-process mock) |
| `ai disconnect` | Close connection |
| `ai status` | Bridge and job counters |
| `ai ask <prompt>` | Create and run a job |
| `ai jobs` | Job table |
| `ai show <job_id>` | Job metadata and response |
| `ai trace <job_id>` | Statechain trace for one job |
| `ai cancel <job_id>` | Cancel queued/running job |
| `ai tools` | Tool permission table |
| `ai allow <tool>` | Allow tool (`Phi3` gate, root authority) |
| `ai deny <tool>` | Deny tool |

## Tool policy

**Allowed (read-only):** `system.state`, `system.version`, `process.list`, `process.show`, `phase.counts`, `trace.read`, `topology.read`, `memory.summary`

**Denied:** `shell.execute`, `kernel.write`, `memory.write`, `process.kill`, `system.reboot`, `system.shutdown`, `log.delete`, `checkpoint.delete`, `motor.control`, `actuator.control`

The AI server may request tools in `tool_requests`; KairOS alone authorizes and executes them.

## Phase lifecycle

- Normal: `Phi1 → Phi2 → Phi0`
- Tool request: `Phi2 → Phi3 → Phi2`
- Denied tool: `Phi2 → Phi3 → Phi0`

## Tests

```bash
cargo test ai_bridge
```

Uses an in-process mock server and a loopback TCP mock — no real model required.

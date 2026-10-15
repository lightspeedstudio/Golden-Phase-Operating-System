# AI Control Layer — Demo Transcript

Tested via `cargo test ai_bridge` and mock transport (`ai connect mock 8000`).

```text
kairos> ai connect mock 8000
AI bridge connected to mock:8000

kairos> ai status
AI BRIDGE STATUS

connected:              yes
server:                 mock:8000
active jobs:            0
queued jobs:            0
completed jobs:         0
...

kairos> ai ask summarize current process phases
AI job 1 created
PID: 2
Phase: Phi0
Status: completed

kairos> ai jobs
JOB   PID   PHASE   STATUS      SERVER          AGE
1     2     Phi0    completed   mock:8000       12ms

kairos> ai show 1
AI JOB 1

Status: completed
Phase: Phi0
PID: 2
Server: mock:8000
Input hash: 7F92....
Output hash: 91AC....
Response: Phi2 has one active process and no recorded violations.

kairos> ai trace 1
[AI:1] Phi1 created
[AI:1] request validated
[AI:1] Phi2 inference started
[AI:1] response received
[AI:1] Phi0 completed

kairos> ai tools
TOOL              PERMISSION
system.state      allowed
...
shell.execute     denied

kairos> ai deny system.state
ai tool denied: system.state

kairos> ai allow system.state
ai tool allowed: system.state

kairos> ai disconnect
AI bridge disconnected
```

External server (management network):

```text
kairos> ai connect 10.0.0.20 8000
AI bridge connected to 10.0.0.20:8000

kairos> ai ask explain the current kernel state
AI job 42 created
PID: 7
Phase: Phi0
Status: completed
```

If connection fails:

```text
kairos> ai connect 10.0.0.99 8000
AI connection failed: connection refused
```

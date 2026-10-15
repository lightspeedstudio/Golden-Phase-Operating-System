# AI Phase Trace — Observable Artifact Archaeology

## What this is

`kairos/scripts/ai_trace_phase.py` encodes **observable textual artifacts** of AI
sessions (conversation excerpts, public instruction snippets, JSONL transcripts)
into Golden Phase Logic (GPL) phase states and reports:

- byte → phase expansion (`golden_phase_runtime.protocol.encoder`)
- GPL two-layer checksum (`gpl_checksum`)
- sliding topology triples (`check_topology`, mode=sliding)
- adjacent pair compose transitions
- exploratory DFT on byte values and phase-index streams (proxy rhythm only)

This is a **first step toward Layer 12 trace parity** applied to AI-visible
bytes — the same codec and topology tools used by KairOS host/QEMU paths.

## What this is NOT

| Claim | Status |
|-------|--------|
| Dump my full Cursor/Claude system prompt | **Not possible** — no retrievable prompt string in agent context |
| Read LLM hidden states or weights | **Not possible** without model-internal exports |
| Decode "why the model thought X" from phase states | **Not possible** — phase encoding is lossless on *bytes*, not on cognition |
| FFT of neural net → AI thoughts | **Sci-fi** without activation/logit dumps from a model you control |

Phase states here are a **transport view of UTF-8 text**, not a window into
transformer internals.

## What IS available as input

1. **Conversation transcript** — e.g. Cursor agent JSONL under
   `~/.cursor/projects/.../agent-transcripts/<id>/<id>.jsonl`
2. **User rules** — visible in session context (partial)
3. **Tool descriptions** — partially visible in context
4. **Assistant output trace** — the text we produce (observable)
5. **Public prompt research** — published snippets only (e.g. Anthropic system
   prompt release notes: https://docs.anthropic.com/en/release-notes/system-prompts)

## Usage

```bash
# From repo root
python kairos/scripts/ai_trace_phase.py kairos/scripts/samples/ai_trace_sample.txt

# JSONL transcript (truncated for speed)
python kairos/scripts/ai_trace_phase.py path/to/session.jsonl --max-bytes 4096

# Public snippet sample
python kairos/scripts/ai_trace_phase.py kairos/scripts/samples/public_claude_snippet.txt
```

Outputs:

- `<input>.phase_trace.json` — full structured report
- `<input>.phase_trace.html` — simple SVG charts (histogram, proxy FFT)

## Interpreting results

- **Phase histogram** — distribution of Φ0–Φ3 after 2-bit expansion; reflects
  byte entropy, not semantics.
- **Topology (same vs reflection)** — associativity classification of sliding
  triples via `compose`; sensitive to local rhythm in the encoded stream.
- **GPL (acc, chi)** — compose fold + branch-sheet sum; fingerprint of the
  whole sequence (analogous to `protocol_tests.rs` / Python reference).
- **Proxy FFT** — periodic structure in bytes or phase indices; useful for
  comparing runs, not for reading thoughts.

## Scientifically possible next steps

1. Export **logits, attention, or activation tensors** from a model you own →
   serialize to bytes → same pipeline (still interpretive, but grounded).
2. **Layer 12 trace parity** — run identical artifact through host script,
   KairOS `protocol::encoder`, and Verilog sim; compare trajectories.
3. Wire KairOS **AI bridge trace** (`kernel/ai/trace.rs`) alongside byte
   encoding for operator-session correlation.

## Related KairOS modules

| Module | Role |
|--------|------|
| `golden_phase_runtime/protocol/encoder.py` | byte ↔ phase (authority for Python) |
| `kairos/src/protocol/encoder.rs` | same codec in Rust |
| `golden_phase_runtime/protocol/topology_checker.py` | sliding/blocked triple topology |
| `golden_phase_runtime/protocol/checksum.py` | GPL two-layer checksum |
| `kairos/src/kernel/ai/trace.rs` | Phi3-gated AI job phase transitions |
| `kairos/docs/ROADMAP.md` Layer 12 | host / QEMU / Verilog one trace |

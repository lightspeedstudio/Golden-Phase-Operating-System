# KairOS StateClock Plan

## Dual-Clock Geometry Using the 10-Sector Golden Atlas

**Status:** Draft for external review  
**Date:** 2026-07-06

---

## 1. Purpose

KairOS needs a time model that fits its mission-state design. A normal operating system mainly tracks physical time: ticks, milliseconds, timers, and scheduling intervals. KairOS needs two kinds of time:

1. **Chronos** — physical/runtime time
2. **Kairos** — meaningful mission-state/event time

The goal is not to replace the existing four phase states. The goal is to add a finer coordinate layer above them so the OS can describe where it is in a mission cycle, when state changes matter, and how state evolves under stress.

The proposed model uses a 10-sector golden atlas:

```text
φ = k · (π / 5) + ε
x = 2 cos(φ / 5)
```

This gives KairOS:

```text
Chronos = continuous angle φ
Kairos  = discrete sector/event index k
ε       = fine offset inside a sector
x       = shared readable coordinate on [-2, 2]
```

---

## 2. Core Idea

The diagram should be treated as a **coordinate atlas**, not as one simple tick counter.

The pieces map like this:

| Diagram piece | KairOS role | Meaning |
|---------------|-------------|---------|
| `φ` on the circle, from `−π` to `+π` | Chronos | Continuous angle: where physical/runtime time is on the cycle |
| `k` sector index, roughly `−5` to `+5` | Kairos | Discrete event sector: which meaningful state zone the system is in |
| `ε` offset inside a sector | Kairos fine step | Optional fine-grain sub-position inside a sector |
| `x = 2 cos(φ / 5)` | Shared coordinate | A readable scalar coordinate on `[-2, 2]` |

Chronos advances because time passes.

Kairos advances because something meaningful happens:

```text
phase transition
syscall
spawn
fault
correction
checkpoint
sector crossing
watchdog event
operator command
```

So Chronos is timer-driven. Kairos is event-driven.

---

## 3. Relationship to Current KairOS

KairOS already has a four-state kernel algebra:

```text
Φ0 = sleep / free / idle
Φ1 = ready / user / mapped
Φ2 = running / active
Φ3 = kernel / locked / privileged
```

These come from the core Golden Phase Runtime equation:

```text
y⁴ − φy² + α = 0
```

The 10-sector atlas should not replace these four states.

Instead:

```text
Layer 1: Φ0–Φ3
compose, reflect, scheduler, memory tags, PhaseFS, statechain

Layer 2: 10 sectors
stateclock geometry, mission zones, display, craft lab zones

Layer 3: x = 2 cos(φ / 5)
shared golden coordinate for logs, dashboard, and mission-state display
```

The four phase states remain the kernel truth.

The 10-sector atlas becomes the stateclock/map layer.

---

## 4. Chronos

Chronos is the continuous time-like clock.

It stores:

```text
chronos_tick
φ
Δφ
x
```

Each Chronos tick advances the angle:

```text
φ = φ + Δφ
```

A simple step size could be:

```text
Δφ = π / (5 · R)
```

where `R` is the number of physical/runtime ticks per sector.

Then the shared coordinate is recomputed:

```text
x = 2 cos(φ / 5)
```

This makes Chronos smooth and cyclic.

The rate along the shared coordinate is:

```text
dx/dφ = −(2/5) sin(φ / 5)
```

That means the system does not move through `x` at a flat binary rate. It moves through a curved golden coordinate.

---

## 5. Kairos

Kairos is the meaningful event clock.

It stores:

```text
kairos_tick
sector_k
epsilon
last_event
last_sector_crossing
```

Kairos advances when state matters:

```text
process state change
memory lock/unlock
filesystem lock/write/delete
syscall
VM trap
watchdog correction
craft subsystem fault
checkpoint seal
operator phase command
```

Kairos may also advance when Chronos crosses a sector boundary.

The sector formula is:

```text
φ = k · (π / 5) + ε
```

So:

```text
k = nearest sector index for φ
ε = φ − k · (π / 5)
```

Chronos answers:

```text
Where are we in continuous runtime?
```

Kairos answers:

```text
Which meaningful state sector are we in?
```

---

## 6. Seam behavior

The diagram's seam behavior is important.

When `φ` wraps from `+π` to `−π`, the shared coordinate should not jump violently. The coordinate:

```text
x = 2 cos(φ / 5)
```

keeps the seam readable and smooth in the golden map.

This gives KairOS a useful story:

```text
Chronos can wrap.
Kairos can step.
The shared coordinate remains interpretable.
```

That helps with mission-state replay, dashboards, and statechain logs.

---

## 7. Mapping sectors to phase states

The 10 sectors should be mapped onto the four kernel states by a fixed table.

This table should be defined once in the stateclock subsystem and imported everywhere else.

Example design direction:

```text
Φ0 → idle / outer / stable sectors
Φ1 → ready / approaching transition sectors
Φ2 → active / mission work sectors
Φ3 → kernel / locked / seam / correction sectors
```

This mapping is a design choice and should be tested.

Important rule:

```text
Do not create 10 different compose tables.
Do not replace compose.
Do not let the 10-sector layer reimplement the four-state kernel algebra.
```

The 10-sector atlas is a clock/coordinate layer, not a second kernel.

---

## 8. Proposed data model

Add a StateClock type:

```text
StateClock {
    chronos_tick: u64,
    kairos_tick: u64,

    phi: fixed_or_float,
    delta_phi: fixed_or_float,

    sector_k: i32,
    epsilon: fixed_or_float,

    x: fixed_or_float,

    proper_tick: u64,
    state_tick: u64,
    dilation_factor: fixed_or_rational,
    skew: i64,
}
```

**Chronos fields:**

```text
chronos_tick
phi
delta_phi
x
```

**Kairos fields:**

```text
kairos_tick
sector_k
epsilon
state_tick
```

**Dual-clock fields:**

```text
proper_tick
dilation_factor
skew
```

---

## 9. StateClock API

Suggested API:

```text
StateClock::new()
StateClock::advance_chronos()
StateClock::record_kairos_event(event)
StateClock::update_sector_from_phi()
StateClock::set_dilation_factor(value)
StateClock::skew()
StateClock::resync()
StateClock::snapshot()
```

Required behavior:

```text
advance_chronos() updates φ and x
record_kairos_event() increments kairos_tick
sector_k updates from φ
epsilon tracks offset inside sector
dilation_factor controls state_tick vs proper_tick
skew reports drift between proper time and mission-state time
```

---

## 10. Shell output target

Update the shell command:

```text
state
```

Example output:

```text
kairos> state

KairOS StateClock

chronos:
  tick: 128
  phi: 1.256 rad
  x: 1.412

kairos:
  tick: 17
  sector: +2
  epsilon: 0.03

dual clock:
  proper_tick: 128
  state_tick: 117
  dilation_factor: 0.914
  skew: -11

kernel phase:
  current: Φ2 running
```

Also expose in:

```text
dashboard
top
watchdog
root
statechain event logs
```

---

## 11. Statechain integration

Every important clock event should be logged into the statechain in hosted mode:

```text
chronos_tick
kairos_event
sector_crossing
dilation_changed
skew_detected
resync
watchdog_correction
```

Each event should include:

```text
previous clock state
new clock state
φ
k
ε
x
proper_tick
state_tick
dilation_factor
skew
event reason
```

This gives KairOS a replayable dual-clock history.

---

## 12. Watchdog integration

The watchdog should use the StateClock to detect mission-state problems:

```text
state_tick not advancing
proper_tick advancing too far ahead
dilation_factor outside safe range
large skew
sector stuck too long
Φ3 locked state stuck across too many Kairos events
```

Possible actions:

```text
monitor
warn
quarantine
safe mode
phase correction
checkpoint
```

The watchdog should not silently fix things. Any correction should be audited.

---

## 13. Craft lab integration

The StateClock fits directly into the craft mission-state lab.

Craft pseudo-processes:

```text
nav
propulsion
power
thermal
comms
guidance
payload
watchdog
```

The craft fixture can show each subsystem against:

```text
Φ-state
sector_k
chronos tick
kairos tick
skew
last correction
```

Example:

```text
kairos> craft status

SUBSYSTEM    PID   PHASE   SECTOR   HEALTH
nav          2     Φ2      +2       nominal
propulsion   3     Φ1      +1       waiting
power        4     Φ3      +5       locked/fault
watchdog     9     Φ2      +3       correcting
```

---

## 14. What this should be used for

**Use** the 10-sector atlas for:

```text
Chronos angle φ
Kairos sector index k
shared x coordinate
mission-state dashboard
stateclock display
craft fixture zones
statechain event context
watchdog skew/correction logic
```

**Do not use** it for:

```text
replacing Φ0–Φ3
creating 10 compose tables
claiming physical time dilation without tests
hiding math inside every syscall
rewriting the whole kernel
```

---

## 15. Phase 1 implementation plan

Phase 1 should be small and testable.

**Files:**

```text
src/stateclock/mod.rs
src/stateclock/clock.rs
src/stateclock/geometry.rs
src/stateclock/event.rs
src/userland/state_cmd.rs
```

**Implement:**

```text
StateClock struct
chronos tick
kairos event tick
φ update
sector_k calculation
epsilon calculation
x = 2 cos(φ / 5)
state command
dashboard display hook
basic statechain event hook in hosted mode
```

**Tests:**

```text
chronos tick advances
φ updates correctly
x computes from φ
sector_k computes from φ
epsilon computes from φ
kairos tick advances only on event
dilation factor changes state_tick behavior
skew is computed
state command prints chronos/kairos/x
statechain records clock event in hosted mode
old tests still pass
QEMU smoke still passes
```

---

## 16. Summary

The 10-sector golden atlas should become the geometry of the KairOS StateClock.

The model is:

```text
Chronos drives φ.
Kairos drives k.
ε gives fine position inside the sector.
x = 2 cos(φ / 5) gives one shared golden coordinate.
```

This gives KairOS a clean dual-clock system:

```text
physical/runtime time
+
meaningful mission-state time
+
a continuous golden coordinate connecting them
```

The four-state kernel remains the foundation. The 10-sector atlas becomes the mission-state clock geometry above it.

---

*Draft — feedback welcome before implementation.*

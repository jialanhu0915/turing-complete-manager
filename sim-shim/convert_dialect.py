#!/usr/bin/env python3
"""Convert a replay.nim-era DSL (old dialect) to the CURRENT compile.dll dialect.

The old dialect (replay.nim, 2026-07-15) differs from what the current
compile.dll / Turing Complete.exe (2026-08-06) accepts in several ways:

  1. Enums:
     - StateIndex: 15 members starting `sim_tick` -> 16 members starting
       `sim_address, sim_cycle, sim_target_cycle, ...` (matches game's current
       sim_state layout).
     - CommandIndex: `ctl_tick_speed_ms` -> `ctl_cycle_speed_ms`.
  2. Identifiers: tick -> cycle throughout (`sim_tick`->`sim_cycle`,
     `#TICK_PAST_FAIL`->`#CYCLE_PAST_FAIL`, `get_target_tick`->`get_target_cycle`,
     `nanos_per_tick`->`nanos_per_cycle`, `burst_ticks`->`burst_cycles`, ...).
  3. Nested defs: the OLD dialect defined `get_input`/`check_output` INSIDE the
     `run_sim: while true` block. The CURRENT compiler does NOT support calling
     a nested def from another function (cross-function nested def), so these
     are moved to TOP-LEVEL.
  4. `#if in_scope(...){...}` inside `mode_run` breaks nested-def registration
     in the current compile.dll -> the block is dropped (UI callbacks are
     optional; resolved at codegen time by the game).
  5. Preamble pointer vars must be COMPACT (single spaces). The game's aligned
     output (`var commands                          = Ptr 0x1000000`) triggers a
     scope-registration bug in compile.dll.
  6. Definition ORDER: the current compiler's nested-def registration breaks
     when the helper section (ui_* / get_* / ComponentType / set_error /
     reset_sim) is defined BEFORE the circuit defs (get_input / check_output /
     mode_run / mode_refresh). Emit circuit defs FIRST, helpers LAST
     (empirically verified working order).

Circuit logic (mode_refresh component updates, get_input/check_output test
logic) is left byte-for-byte identical; only the DSL scaffold is adapted.

Usage: python convert_dialect.py [in.dsl] [out.dsl]
  default in=and_gate.dsl  out=and_gate_current.dsl
"""
import re
import sys

SRC = sys.argv[1] if len(sys.argv) > 1 else "and_gate.dsl"
DST = sys.argv[2] if len(sys.argv) > 2 else "and_gate_current.dsl"

OLD_STATEINDEX = (
    "type StateIndex Enum[sim_tick, sim_target_tick, sim_test_result, sim_last_command_id, "
    "sim_error_component, sim_short_circuit_component_id_1, sim_short_circuit_component_id_2, "
    "sim_short_circuit_pin_1, sim_short_circuit_pin_2, sim_short_circuit_top_level_permanent_id_1, "
    "sim_short_circuit_top_level_permanent_id_2, sim_short_circuit_value_1, sim_short_circuit_value_2, "
    "sim_short_circuit_any_top_level_wire_id, sim_running]"
)
NEW_STATEINDEX = (
    "type StateIndex Enum[sim_address, sim_cycle, sim_target_cycle, sim_test_result, "
    "sim_last_command_id, sim_error_component, sim_short_circuit_component_id_1, "
    "sim_short_circuit_component_id_2, sim_short_circuit_pin_1, sim_short_circuit_pin_2, "
    "sim_short_circuit_top_level_permanent_id_1, sim_short_circuit_top_level_permanent_id_2, "
    "sim_short_circuit_value_1, sim_short_circuit_value_2, sim_short_circuit_any_top_level_wire_id, "
    "sim_running]"
)
OLD_COMMANDINDEX = (
    "type CommandIndex Enum[ctl_command, ctl_command_id, ctl_tick_speed_ms, ctl_exit, "
    "ctl_level_manual_input, ctl_level_manual_input_id, ctl_test]"
)
NEW_COMMANDINDEX = (
    "type CommandIndex Enum[ctl_command, ctl_command_id, ctl_cycle_speed_ms, ctl_exit, "
    "ctl_level_manual_input, ctl_level_manual_input_id, ctl_test]"
)

RENAMES = [
    ("sim_target_tick", "sim_target_cycle"),
    ("sim_tick", "sim_cycle"),
    ("var tick = -1", "var cycle = -1"),  # global state var backing `.tick`
    ("ctl_tick_speed_ms", "ctl_cycle_speed_ms"),
    ("#TICK_PAST_FAIL", "#CYCLE_PAST_FAIL"),
    ("get_target_tick", "get_target_cycle"),
    ("run_target_tick", "run_target_cycle"),
    ("burst_target_tick", "burst_target_cycle"),
    ("orig_start_tick", "orig_start_cycle"),
    ("nanos_per_tick", "nanos_per_cycle"),
    ("burst_ticks", "burst_cycles"),
    ("ticks_at_start", "cycles_at_start"),
    ("target_tick", "target_cycle"),
]

# Compact versions of the game's aligned preamble pointer vars. The game emits
# these with heavy alignment spacing; compile.dll mis-registers vars declared on
# such lines (a genuine scope bug), so we emit them compactly.
PREAMBLE_COMPACT = [
    "var commands = Ptr 0x1000000",
    "var settings = Ptr 0x1000010",
    "var input_replay = [U64] 0x1000020",
    "var output_history_pins = Ptr 0x1000030",
    "var error_buffer = Ptr 0x1000040",
    "var ui_buffer = Ptr 0x1000050",
    "const #SIMULATION_STATE = Ptr 0x1000060",
    "const #SIMULATION_KEYBOARD_CHARACTER = Ptr 0x1000070",
    "const #SIMULATION_KEYBOARD_COORDINATE = Ptr 0x1000080",
]


def extract_block(lines, i):
    """Extract a brace-balanced block (def / run_sim:) starting at line i.

    Returns (block_lines, index_of_first_line_after_block).
    """
    depth = 0
    block = []
    j = i
    while j < len(lines):
        block.append(lines[j])
        depth += lines[j].count("{") - lines[j].count("}")
        if depth <= 0 and lines[j].strip().endswith("}") and "run_sim:" not in lines[j]:
            break
        j += 1
    return block, j


def main():
    with open(SRC, encoding="utf-8") as f:
        text = f.read()

    assert OLD_STATEINDEX in text, "StateIndex enum not found — update script?"
    assert OLD_COMMANDINDEX in text, "CommandIndex enum not found — update script?"
    text = text.replace(OLD_STATEINDEX, NEW_STATEINDEX)
    text = text.replace(OLD_COMMANDINDEX, NEW_COMMANDINDEX)
    for old, new in RENAMES:
        text = text.replace(old, new)
    # `.tick` dotted access -> `.cycle` (after identifier renames above).
    text = re.sub(r"\.tick\b", ".cycle", text)

    lines = text.split("\n")

    # ── Extract nested get_input / check_output from run_sim ────────────────
    get_input_block = check_output_block = None
    in_run_sim = False
    i = 0
    while i < len(lines):
        s = lines[i].strip()
        if s.startswith("run_sim:"):
            in_run_sim = True
        elif in_run_sim and s.startswith("def get_input("):
            blk, j = extract_block(lines, i)
            get_input_block = [b[4:] if b.startswith("    ") else b for b in blk]  # dedent
            i = j + 1
            continue
        elif in_run_sim and s.startswith("def check_output("):
            blk, j = extract_block(lines, i)
            check_output_block = [b[4:] if b.startswith("    ") else b for b in blk]
            i = j + 1
            continue
        i += 1
    assert get_input_block is not None and check_output_block is not None, \
        "nested get_input/check_output not found in run_sim"

    # ── Remove nested get_input/check_output from run_sim ───────────────────
    stripped = []
    in_run_sim = False
    i = 0
    while i < len(lines):
        s = lines[i].strip()
        if s.startswith("run_sim:"):
            in_run_sim = True
        if in_run_sim and (s.startswith("def get_input(") or s.startswith("def check_output(")):
            _, j = extract_block(lines, i)
            i = j + 1  # extract_block's j is the inclusive closing `}` line
            continue
        stripped.append(lines[i])
        i += 1
    lines = stripped

    # ── Drop `#if in_scope(...)` blocks inside mode_run ─────────────────────
    # (they break nested-def registration; UI callbacks resolved at codegen).
    cleaned = []
    i = 0
    in_mode_run = False
    mode_depth = 0
    while i < len(lines):
        s = lines[i].strip()
        if s.startswith("def mode_run("):
            in_mode_run = True
            mode_depth = 1
            cleaned.append(lines[i])
            i += 1
            continue
        if in_mode_run:
            # track brace depth; a `#if in_scope(` at depth 1 inside mode_run is dropped
            if s.startswith("#if in_scope("):
                j = i
                depth = mode_depth
                while j < len(lines):
                    depth += lines[j].count("{") - lines[j].count("}")
                    if depth <= mode_depth and lines[j].strip() == "}":
                        break
                    j += 1
                i = j + 1
                continue
            cleaned.append(lines[i])
            mode_depth += lines[i].count("{") - lines[i].count("}")
            if mode_depth <= 0:
                in_mode_run = False
            i += 1
            continue
        cleaned.append(lines[i])
        i += 1
    lines = cleaned

    # ── Compact the aligned preamble pointer vars ───────────────────────────
    def compact_preamble(line):
        s = line.strip()
        for c in PREAMBLE_COMPACT:
            cname = c.split(" ")[0] + " " + c.split(" ")[1]  # e.g. "var commands"
            if s.startswith(cname + " ") or s.startswith(cname + "="):
                return c
        return line
    lines = [compact_preamble(l) for l in lines]

    # ── Reassemble in the empirically-verified WORKING order ────────────────
    # Canonical order (probe_n23/pb_AG, compiles cleanly through compile.dll):
    #   types -> basic defs -> get_input/check_output (top-level) -> state vars
    #   -> mode_run -> mode_refresh -> set_error/reset_sim -> helpers -> run_sim
    def defname(s):
        m = re.match(r"def\s+([a-zA-Z_0-9]+)\(", s)
        return m.group(1) if m else None

    blocks = []
    i = 0
    while i < len(lines):
        s = lines[i].strip()
        if s.startswith("run_sim:"):
            blk, j = extract_block(lines, i)
            blocks.append(("run_sim", blk))
            i = j + 1
        elif s.startswith("def "):
            blk, j = extract_block(lines, i)
            blocks.append(("def", defname(s), blk))
            i = j + 1
        elif s.startswith("type "):
            if s.endswith("{"):
                # multi-line struct type: capture through closing `}`
                blk, j = extract_block(lines, i)
                blocks.append(("type", s, blk))
                i = j + 1
            else:
                # single-line enum type
                blocks.append(("type", s, [lines[i]]))
                i += 1
        elif s.startswith("const "):
            blocks.append(("const", s, [lines[i]]))
            i += 1
        elif s.startswith("var "):
            blocks.append(("var", s, [lines[i]]))
            i += 1
        else:
            i += 1  # blank/comment

    # Categorize into the canonical order buckets.
    types = [b for b in blocks if b[0] == "type" and not b[1].startswith("type ComponentType")]
    basic = []          # get_command, get_setting, set_setting, max, set_text, #CYCLE_PAST_FAIL
    input_defs = []     # get_input, check_output (top-level test logic)
    runtime_defs = []   # mode_run, mode_refresh (circuit runtime)
    reset_defs = []     # set_error, reset_sim
    helpers = []        # everything else (ComponentType enum, get_*, ui_*, ...)
    state_vars = []     # var cycle, commands, ..., level_input/level_output
    trailing_vars = []  # last_level_manual_input, time_component_last, manual_input_seen
    run_sim = None

    BASIC = {"get_command", "get_setting", "set_setting", "max", "set_text"}
    INPUT_DEFS = {"get_input", "check_output"}   # circuit defs part 1
    RUNTIME_DEFS = {"mode_run", "mode_refresh"}  # circuit defs part 2
    RESET = {"set_error", "reset_sim"}

    for b in blocks:
        kind = b[0]
        if kind == "run_sim":
            run_sim = b
        elif kind == "def":
            name = b[1]
            if name in BASIC:
                basic.append(b)
            elif name in INPUT_DEFS:
                input_defs.append(b)
            elif name in RUNTIME_DEFS:
                runtime_defs.append(b)
            elif name in RESET:
                reset_defs.append(b)
            else:
                helpers.append(b)

        elif kind == "type":
            s = b[1]
            if s.startswith("type ComponentType"):
                # ComponentType must NOT come before the circuit defs: the current
                # compile.dll breaks nested-def registration in mode_run when the
                # helper section precedes it. Emit it with the helpers (last).
                helpers.append(b)
            # other types already collected above (list comprehension)
        elif kind == "const":
            s = b[1]
            if s.startswith("const #SIMULATION"):
                # SIMULATION_* pointers belong with the state vars (working order).
                state_vars.append(b)
            else:
                # #CYCLE_PAST_FAIL goes with basic (before circuit defs)
                basic.append(b)
        elif kind == "var":
            s = b[1]
            if "last_level_manual_input" in s or "time_component_last" in s or "manual_input_seen" in s:
                trailing_vars.append(b)
            else:
                state_vars.append(b)

    # get_input/check_output were extracted out of run_sim and stripped; add them
    # back as top-level blocks in the input_defs slot.
    input_defs = ([("def", "get_input", get_input_block)] +
                  [("def", "check_output", check_output_block)] + input_defs)

    def emit(blocks):
        # No blank lines between/within blocks: the replay.nim-era double-blank
        # formatting breaks compile.dll's nested-def registration inside mode_run
        # (verified 2026-08-08 — blank-line-free output compiles, spaced fails).
        out = []
        for b in blocks:
            out.append("\n".join(line.rstrip() for line in b[2] if line.strip()))
        return out

    final = []
    final.append("// and_gate DSL converted to CURRENT compile.dll dialect (see convert_dialect.py).")
    final.extend(emit(types))
    # Order within groups is load-bearing for the current compile.dll:
    #   basic: get_command, get_setting, set_setting, max, set_text, #CYCLE_PAST_FAIL
    #   runtime: mode_run BEFORE mode_refresh
    basic.sort(key=lambda b: ["get_command", "get_setting", "set_setting", "max", "set_text"].index(b[1])
               if b[1] in ("get_command", "get_setting", "set_setting", "max", "set_text") else 99)
    runtime_defs.sort(key=lambda b: 0 if b[1] == "mode_run" else 1)
    final.extend(emit(basic))
    final.extend(emit(input_defs))       # get_input, check_output (top-level)
    final.extend(emit(state_vars))       # cycle, commands, settings, SIMULATION_*, level vars
    final.extend(emit(runtime_defs))     # mode_run, mode_refresh
    final.extend(emit(reset_defs))       # set_error, reset_sim
    final.extend(emit(helpers))          # ComponentType, get_*, ui_*, ...
    final.extend(emit(trailing_vars))    # last_level_manual_input, time_component_last, manual_input_seen
    if run_sim:
        final.append("\n".join(line.rstrip() for line in run_sim[1] if line.strip()))

    with open(DST, "w", encoding="utf-8") as f:
        f.write("\n".join(final))
    print(f"Wrote {DST}: {len(final)} lines")

    # Sanity checks.
    src_body = "\n".join(final)
    assert "def get_input(tick: Int) Input {" in src_body
    assert "def check_output(tick: Int, input: Input, output: Output) TestResult {" in src_body
    assert "run_sim: while true {" in src_body
    # run_sim's `#if in_scope(...)` blocks are kept (harmless; compile fine).
    # mode_run's must be gone (breaks nested-def registration).
    assert not re.search(r"def mode_run\(\).*?#if in_scope", src_body, re.S), \
        "mode_run must not contain #if in_scope blocks"
    assert ".tick" not in src_body, "leftover .tick"
    # compact preamble present
    assert "var commands = Ptr 0x1000000" in src_body
    print("  order: circuit defs before helpers, preamble compact, #if dropped")


if __name__ == "__main__":
    main()

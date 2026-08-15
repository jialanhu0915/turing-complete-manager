//! DSL generator: `Circuit` → current-dialect Nim DSL that `compile.dll`
//! accepts and `exec` can drive.
//!
//! The output is assembled from:
//! - a **static skeleton** (the proven `and_gate_gen.dsl` structure, verified
//!   to compile + run in Phase 1) with the level-specific and circuit-specific
//!   sections swapped out;
//! - a per-level **test template** (`LevelTemplate`, authored in `levels.rs`):
//!   Input/Output structs, `get_input` (input generation), `check_output`
//!   (expected-value comparison);
//! - a **generated circuit-sim** section: each component, in topological order,
//!   emits its per-cycle computation (gates read upstream `vidN` vars, level
//!   I/O reads/writes `.level_input` / `.level_output`).
//!
//! The output must follow the current dialect's 5 restrictions (compact
//! preamble, top-level defs, no `#if` in `mode_run`, no blank lines, circuit
//! defs before helpers) — the skeleton is byte-for-byte the verified template.

use std::collections::HashMap;

use crate::circuit::model::{Circuit, Component, Point};
use crate::circuit::pins::{self, Connectivity, PinDir};

/// Per-level test template (the level's I/O contract + test oracle).
///
/// Built from `test.si` by [`crate::dll::test_si::parse`] (the game's per-level
/// test spec) — no hand-authored templates.
#[derive(Debug, Clone)]
pub struct LevelTemplate {
    /// `type Input { ... }` block.
    pub input_struct: String,
    /// `type Output { ... }` block.
    pub output_struct: String,
    /// Compacted module decls + helper defs + `get_input` + `check_output`
    /// (the game's test logic, verbatim after blank-line removal).
    pub test_defs: String,
    /// `(field_name, field_type)` for level-input ports, in component order.
    pub input_fields: Vec<(String, String)>,
    /// `(field_name, field_type)` for level-output ports, in component order.
    pub output_fields: Vec<(String, String)>,
    /// z-flag fields set to `false` after the outputs (`output_is_z`, ...).
    pub output_z_fields: Vec<String>,
}

// ─── Component kind → DSL facts ────────────────────────────────────────────

/// circuit.data kind → ComponentType enum index (for `#COUNTS`).
/// Only kinds the generator can currently simulate are listed.
fn kind_to_enum(kind: u16) -> Option<usize> {
    Some(match kind {
        3 => 2,   // com_not_bit
        4 => 3,   // com_and_bit
        5 => 4,   // com_and_3_bit
        6 => 5,   // com_nand_bit
        7 => 6,   // com_or_bit
        8 => 7,   // com_or_3_bit
        9 => 8,   // com_nor_bit
        10 => 9,  // com_xor_bit
        11 => 10, // com_xnor_bit
        16 => 15, // com_maker_bit_8
        17 => 16, // com_splitter_bit_8
        18 => 17, // com_not_word
        19 => 18, // com_or_word
        20 => 19, // com_and_word
        21 => 20, // com_nand_word
        22 => 21, // com_xor_word
        23 => 22, // com_nor_word
        24 => 23, // com_xnor_word
        29 => 28, // com_neg
        60 => 56, // com_level_input_1_pin
        61 => 57, // com_level_input_word
        63 => 59, // com_level_input_2_pin
        68 => 62, // com_level_output_1_pin
        69 => 63, // com_level_output_word
        _ => return None,
    })
}

/// circuit.data kind → DSL `ComponentType` name (for the `//` comments).
fn kind_to_name(kind: u16) -> Option<&'static str> {
    Some(match kind {
        3 => "com_not_bit",
        4 => "com_and_bit",
        5 => "com_and_3_bit",
        6 => "com_nand_bit",
        7 => "com_or_bit",
        8 => "com_or_3_bit",
        9 => "com_nor_bit",
        10 => "com_xor_bit",
        11 => "com_xnor_bit",
        16 => "com_maker_bit_8",
        17 => "com_splitter_bit_8",
        18 => "com_not_word",
        19 => "com_or_word",
        20 => "com_and_word",
        21 => "com_nand_word",
        22 => "com_xor_word",
        23 => "com_nor_word",
        24 => "com_xnor_word",
        29 => "com_neg",
        60 => "com_level_input_1_pin",
        61 => "com_level_input_word",
        63 => "com_level_input_2_pin",
        68 => "com_level_output_1_pin",
        69 => "com_level_output_word",
        _ => return None,
    })
}

/// circuit.data kinds of level input / output components.
fn is_level_input(kind: u16) -> bool {
    matches!(kind, 60 | 61 | 62 | 63 | 64 | 65 | 106)
}
fn is_level_output(kind: u16) -> bool {
    matches!(kind, 40 | 58 | 68 | 69 | 70 | 73 | 74 | 75 | 77 | 112)
}

/// Per-cycle simulation expressions for a combinational component, given the
/// vid slots of its input pins' drivers (in input-pin order, each with its bit
/// width) and the component's result width. Returns one expression per output
/// pin (most gates have one; a splitter has one per output bit). `None` input
/// = undriven → constant 0, matching the game's convention.
///
/// DSL operator forms were extracted from the game's own generated DSL in
/// `replay.nim` (e.g. AND = `(U1 a) & (U1 b)`, NOT = `U1 ~(U1 a)`, splitter =
/// `(input >> i) & 1`, maker = `U8 (b0 | b1<<1 | ...)`).
fn component_exprs(
    kind: u16,
    inputs: &[(Option<usize>, i64)],
    output_count: usize,
    width: i64,
) -> Result<Vec<String>, String> {
    let t = format!("U{width}");
    let ref_of = |src: &(Option<usize>, i64)| match src.0 {
        Some(slot) => format!("(U{} vid{slot})", src.1),
        None => format!("(U{} 0x0)", src.1),
    };
    let n = |expected: usize| {
        if inputs.len() != expected {
            return Err(format!(
                "KIND_{kind}_EXPECTS_{expected}_INPUTS|got={}",
                inputs.len()
            ));
        }
        Ok(())
    };

    // Splitter: one word input → N bit outputs (bit i = (in >> i) & 1).
    if kind == 17 {
        n(1)?;
        let inn = ref_of(&inputs[0]);
        let mut out = Vec::with_capacity(output_count);
        for i in 0..output_count {
            if i == 0 {
                out.push(format!("U1 ({inn} & 1)"));
            } else {
                out.push(format!("U1 ({inn} >> {i} & 1)"));
            }
        }
        return Ok(out);
    }
    // Maker: N bit inputs → one word output (b0 | b1<<1 | b2<<2 | ...). Each
    // bit must be widened to `t` BEFORE shifting, else a U1 shift overflows.
    if kind == 16 {
        let terms: Vec<String> = inputs
            .iter()
            .enumerate()
            .map(|(i, src)| format!("(({t} {}) << {i})", ref_of(src)))
            .collect();
        return Ok(vec![format!("{t} ({})", terms.join(" | "))]);
    }

    let expr = match kind {
        3 => {
            n(1)?;
            format!("{t} ({t} ~{})", ref_of(&inputs[0]))
        }
        4 => {
            n(2)?;
            format!("{t} {} & {}", ref_of(&inputs[0]), ref_of(&inputs[1]))
        }
        5 => {
            n(3)?;
            format!(
                "{t} {} & {} & {}",
                ref_of(&inputs[0]),
                ref_of(&inputs[1]),
                ref_of(&inputs[2])
            )
        }
        6 => {
            n(2)?;
            format!("{t} ({t} ~({} & {}))", ref_of(&inputs[0]), ref_of(&inputs[1]))
        }
        7 => {
            n(2)?;
            format!("{t} {} | {}", ref_of(&inputs[0]), ref_of(&inputs[1]))
        }
        8 => {
            n(3)?;
            format!(
                "{t} {} | {} | {}",
                ref_of(&inputs[0]),
                ref_of(&inputs[1]),
                ref_of(&inputs[2])
            )
        }
        9 => {
            n(2)?;
            format!("{t} ({t} ~({} | {}))", ref_of(&inputs[0]), ref_of(&inputs[1]))
        }
        10 => {
            n(2)?;
            format!("{t} {} ^ {}", ref_of(&inputs[0]), ref_of(&inputs[1]))
        }
        11 => {
            n(2)?;
            format!("{t} ({t} ~({} ^ {}))", ref_of(&inputs[0]), ref_of(&inputs[1]))
        }
        18 => {
            n(1)?;
            format!("{t} ({t} ~{})", ref_of(&inputs[0]))
        }
        19 => {
            n(2)?;
            format!("{t} {} | {}", ref_of(&inputs[0]), ref_of(&inputs[1]))
        }
        20 => {
            n(2)?;
            format!("{t} {} & {}", ref_of(&inputs[0]), ref_of(&inputs[1]))
        }
        21 => {
            n(2)?;
            format!("{t} ({t} ~({} & {}))", ref_of(&inputs[0]), ref_of(&inputs[1]))
        }
        22 => {
            n(2)?;
            format!("{t} {} ^ {}", ref_of(&inputs[0]), ref_of(&inputs[1]))
        }
        23 => {
            n(2)?;
            format!("{t} ({t} ~({} | {}))", ref_of(&inputs[0]), ref_of(&inputs[1]))
        }
        24 => {
            n(2)?;
            format!("{t} ({t} ~({} ^ {}))", ref_of(&inputs[0]), ref_of(&inputs[1]))
        }
        29 => {
            n(1)?;
            format!("{t} (-{})", ref_of(&inputs[0]))
        }
        other => return Err(format!("UNSUPPORTED_GATE_KIND|{other}")),
    };
    Ok(vec![expr; output_count.max(1)])
}

// ─── Circuit-sim emission ──────────────────────────────────────────────────

/// A reference to an input pin's driving value: the upstream output slot, or a
/// constant 0 if the pin is undriven.
type InputSrc = Option<usize>;

/// The upstream output slot feeding `p`'s position, or `None` if the pin is
/// undriven (no net, or the net has no output pin).
fn driver_slot(
    p: &pins::PositionedPin,
    net_by_pos: &HashMap<Point, usize>,
    networks: &[pins::Net],
    out_slot: &HashMap<(usize, String), usize>,
) -> InputSrc {
    let &net_id = net_by_pos.get(&p.position)?;
    let driver = networks.iter().find(|n| n.id == net_id)?.driver.clone()?;
    out_slot
        .get(&(driver.component_index, driver.name))
        .copied()
}

/// Emit the `mode_run` per-cycle body (one block per component, in topological
/// order). Lines are returned already indented at the `mode_run` body depth.
fn emit_circuit_sim(
    circuit: &Circuit,
    conn: &Connectivity,
    tpl: &LevelTemplate,
) -> Result<String, String> {
    // position → logical network id (all pins of a net share its driver).
    let mut net_by_pos: HashMap<Point, usize> = HashMap::new();
    for net in &conn.networks {
        for p in &net.pins {
            net_by_pos.insert(p.position, net.id);
        }
    }

    let components = &circuit.components;
    let mut out_slot: HashMap<(usize, String), usize> = HashMap::new();
    let mut next_slot = 0usize;
    let mut lines: Vec<String> = Vec::new();

    // Field ↔ component assignment follows the component POSITION (x 升序，再
    // y 升序)，NOT Vec order — 玩家存档里输入组件的 Vec 顺序可能与 test.si 字段
    // 顺序不一致（byte_adder 实测：Vec 顺序 Carry in/A/B，但字段顺序 a/b/carry_in）。
    let mut in_fields: HashMap<usize, (String, String)> = HashMap::new();
    {
        let mut fields = tpl.input_fields.iter();
        let mut idxs: Vec<usize> = components
            .iter()
            .enumerate()
            .filter(|(_, c)| is_level_input(c.kind))
            .map(|(i, _)| i)
            .collect();
        idxs.sort_by_key(|&i| (components[i].position.0, components[i].position.1));
        for idx in idxs {
            let f = fields.next().ok_or("MISSING_INPUT_FIELD")?.clone();
            in_fields.insert(idx, f);
        }
    }
    let mut out_fields: HashMap<usize, (String, String)> = HashMap::new();
    {
        // Output components split by kind: kind 112 = z-flag, others = value.
        // 每组内按位置排序映射到对应字段列表。
        let mut value_fields = tpl.output_fields.iter().cloned();
        let mut z_fields = tpl
            .output_z_fields
            .iter()
            .cloned()
            .map(|z| (z, "Bool".to_string()));
        let mut idxs: Vec<usize> = components
            .iter()
            .enumerate()
            .filter(|(_, c)| is_level_output(c.kind))
            .map(|(i, _)| i)
            .collect();
        // 输出字段顺序由 check_output 决定（byte_adder 实测：carry_out(U1) 先于
        // output(U8)），故按 word_size 升序匹配，再按位置做同宽度消歧。
        idxs.sort_by_key(|&i| {
            (
                components[i].word_size,
                components[i].position.0,
                components[i].position.1,
            )
        });
        for idx in idxs {
            let comp = &components[idx];
            let f = if comp.kind == 112 {
                z_fields
                    .next()
                    .ok_or("MISSING_OUTPUT_Z_FIELD")?
            } else {
                value_fields
                    .next()
                    .ok_or("MISSING_OUTPUT_FIELD")?
            };
            out_fields.insert(idx, f);
        }
    }

    let comment = |ci: usize, comp: &Component| -> String {
        let name = kind_to_name(comp.kind).unwrap_or("com_unknown");
        format!(
            "// {ci} {name} {} {perm} {label}",
            comp.word_size,
            perm = comp.permanent_id,
            label = comp.user_label
        )
    };

    for &ci in &conn.topo_order {
        let comp = &components[ci];
        let pins = pins::positioned_pins(comp, ci);
        let cmt = comment(ci, comp);

        if is_level_input(comp.kind) {
            // One component ↔ one Input port. A 1-pin component reads the whole
            // field; an N-pin component reads the packed field's bits (pin i →
            // bit i, LSB first — verified empirically for and_gate).
            let (fname, ftype) = in_fields
                .get(&ci)
                .ok_or("MISSING_INPUT_FIELD")?
                .clone();
            let out_pins: Vec<_> = pins
                .iter()
                .filter(|p| p.direction != PinDir::Input)
                .collect();
            lines.push(cmt);
            if out_pins.len() == 1 {
                let slot = next_slot;
                next_slot += 1;
                out_slot.insert((ci, out_pins[0].name.clone()), slot);
                lines.push(format!("var vid{slot} = {ftype} .level_input.{fname}"));
            } else {
                for (i, p) in out_pins.iter().enumerate() {
                    let slot = next_slot;
                    next_slot += 1;
                    out_slot.insert((ci, p.name.clone()), slot);
                    lines.push(format!(
                        "var vid{slot} = U1 (.level_input.{fname} >> {i} & 1)"
                    ));
                }
            }
        } else if is_level_output(comp.kind) {
            // One component ↔ one Output port; value read from its input pin's
            // driver. Undriven → constant 0 (broken circuit; check_output's
            // comparison fails against a high expected). `ftype` prefixes it.
            // Z-flag outputs (kind 112) write a Bool directly without prefix.
            let (fname, ftype) = out_fields
                .get(&ci)
                .ok_or("MISSING_OUTPUT_FIELD")?
                .clone();
            let in_pins: Vec<_> = pins
                .iter()
                .filter(|p| p.direction == PinDir::Input)
                .collect();
            lines.push(cmt);
            if comp.kind == 112 {
                // z-flag: Bool field, no ftype prefix（单 pin）。
                let src = in_pins
                    .first()
                    .and_then(|p| driver_slot(p, &net_by_pos, &conn.networks, &out_slot))
                    .map(|s| format!("vid{s}"))
                    .unwrap_or_else(|| "0x0".to_string());
                lines.push(format!(".level_output.{fname} = {src} != 0x0"));
            } else if in_pins.len() == 1 {
                let src = driver_slot(in_pins[0], &net_by_pos, &conn.networks, &out_slot)
                    .map(|s| format!("vid{s}"))
                    .unwrap_or_else(|| "0x0".to_string());
                lines.push(format!(".level_output.{fname} = {ftype} {src}"));
            } else {
                // 多位输出（字输出）：把 N 个 bit 合并成一个字，bit i → << i（同 maker）。
                let terms: Vec<String> = in_pins
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let src = driver_slot(p, &net_by_pos, &conn.networks, &out_slot)
                            .map(|s| format!("vid{s}"))
                            .unwrap_or_else(|| "0x0".to_string());
                        format!("(({ftype} {src}) << {i})")
                    })
                    .collect();
                lines.push(format!(
                    ".level_output.{fname} = {ftype} ({})",
                    terms.join(" | ")
                ));
            }
        } else {
            let inputs: Vec<(InputSrc, i64)> = pins
                .iter()
                .filter(|p| p.direction == PinDir::Input)
                .map(|p| {
                    (
                        driver_slot(p, &net_by_pos, &conn.networks, &out_slot),
                        p.width,
                    )
                })
                .collect();
            let out_pins: Vec<_> = pins
                .iter()
                .filter(|p| p.direction != PinDir::Input)
                .collect();
            // Result width comes from the output pins (a maker_bit_8 has
            // word_size=1 but an 8-bit output), not the component's word_size.
            let out_width = out_pins.first().map(|p| p.width).unwrap_or(comp.word_size);
            let exprs = component_exprs(comp.kind, &inputs, out_pins.len(), out_width)?;
            lines.push(cmt);
            for (i, p) in out_pins.iter().enumerate() {
                let slot = next_slot;
                next_slot += 1;
                out_slot.insert((ci, p.name.clone()), slot);
                lines.push(format!("var vid{slot} = {}", exprs[i]));
            }
        }
    }

    // z-flag fields: every level output is driven, so none is high-Z.
    for zf in &tpl.output_z_fields {
        lines.push(format!(".level_output.{zf} = false"));
    }

    // Indent to the mode_run per-cycle body depth (3 × 4 spaces).
    Ok(lines
        .iter()
        .map(|l| format!("            {l}"))
        .collect::<Vec<_>>()
        .join("\n"))
}

// ─── Static skeleton (verified structure from and_gate_gen.dsl) ────────────

/// The full DSL skeleton with `__MARKERS__` for the level/circuit-specific
/// sections. Byte-for-byte the Phase-1-verified `and_gate_gen.dsl` structure.
const SKELETON: &str = r#"// Generated by turing-complete-manager DSL generator.
type SimCommand Enum[run, refresh, mode_reset, quit_simulation]
type CommandIndex Enum[ctl_command, ctl_command_id, ctl_cycle_speed_ms, ctl_exit, ctl_level_manual_input, ctl_level_manual_input_id, ctl_test]
type StateIndex Enum[sim_address, sim_cycle, sim_target_cycle, sim_test_result, sim_last_command_id, sim_error_component, sim_short_circuit_component_id_1, sim_short_circuit_component_id_2, sim_short_circuit_pin_1, sim_short_circuit_pin_2, sim_short_circuit_top_level_permanent_id_1, sim_short_circuit_top_level_permanent_id_2, sim_short_circuit_value_1, sim_short_circuit_value_2, sim_short_circuit_any_top_level_wire_id, sim_running]
type TestResult Enum[pass, win, fail]
__INPUT_OUTPUT_STRUCTS__
def get_command(idx: CommandIndex) U64 {
    return load(<U64>, .commands + (Int idx) * 8)
}
def get_setting(idx: StateIndex) U64 {
    return load(<U64>, .settings + (Int idx) * 8)
}
def set_setting(idx: StateIndex, value: U64) {
    store(.settings + (Int idx) * 8, value)
}
def max(a: TestResult, b: TestResult) TestResult {
    return TestResult max(U64 a, U64 b)
}
def set_text(text: String, offset: Int) {
    let length = text.len()
    store(.ui_buffer + offset, length)
    var i = 0
    while i < length {
        store(.ui_buffer + offset + 8 + i, U8 text[i])
        i += 1
    }
}
const #CYCLE_PAST_FAIL = 0
__TEST_DEFS__
var commands = Ptr 0x1000000
var settings = Ptr 0x1000010
var input_replay = [U64] 0x1000020
var output_history_pins = Ptr 0x1000030
var error_buffer = Ptr 0x1000040
var ui_buffer = Ptr 0x1000050
const #SIMULATION_STATE = Ptr 0x1000060
const #SIMULATION_KEYBOARD_CHARACTER = Ptr 0x1000070
const #SIMULATION_KEYBOARD_COORDINATE = Ptr 0x1000080
var cycle = -1
var last_command_id = U64 0
var level_input = Input {}
var level_output = Output {}
def mode_run(target_cycle: Int) {
    if get_setting(sim_test_result) == U64 win { return }
    if (get_setting(sim_test_result) == U64 fail && .cycle + 1 >= #CYCLE_PAST_FAIL) { return }
    if get_setting(sim_short_circuit_component_id_1) != 0 { return }
    var halt_run = false
    var original_start_time = Time 0
    var orig_start_cycle = U64 .cycle
    var last_time = original_start_time
    var burst_cycles = 1
    var nanos_per_cycle = U64 0
    var i = 0
    var last_target_speed = U64 0
    var speed = U64 1
    while get_command(ctl_command_id) == .last_command_id && !halt_run && target_cycle > .cycle {
        i += 1
        let target_speed = max(1, get_command(ctl_cycle_speed_ms))
        if target_speed != last_target_speed {
            original_start_time = get_time()
            orig_start_cycle = U64 .cycle
            last_time = original_start_time
            speed = min(U64 get_command(ctl_cycle_speed_ms), 10000000000000) // Cap at 10 ghz to avoid overflow
            last_target_speed = target_speed
        }
        let cycles_at_start = .cycle
        let burst_target = .cycle + burst_cycles
        var burst_target_cycle = min(target_cycle, burst_target)
        def halt() {
            .burst_target_cycle = .cycle
            .halt_run = true
        }
        def handle_test_result(result: TestResult) {
            if result != pass {
                var res = result
                halt() // Halt regardless of wether we are above '#CYCLE_PAST_FAIL'
                set_setting(sim_test_result, U64 res)
            }
        }
        while .cycle < burst_target_cycle {
            .level_input = get_input(.cycle + 1)
__CIRCUIT_SIM__
            let result = check_output(.cycle + 1, .level_input, .level_output)
            handle_test_result(result)
            .cycle += 1 // Do this late as it signals to the front end that it can update
        }
        set_setting(sim_cycle, U64 .cycle)
        let time_now = get_time()
        let time_diff = U64 (time_now - last_time)
        nanos_per_cycle = time_diff / U64 burst_cycles
        if time_now - last_time > 33.ms() {
            // Burst took too long
            burst_cycles = max(1, burst_cycles - 1)
        } else {
            // Burst was too fast
            burst_cycles = burst_cycles + 1
        }
        last_time = time_now
        def get_target_cycle(time_now: Time) U64 {
            let time_diff = U64 (time_now - .original_start_time)
            return .orig_start_cycle + ((time_diff / 1000000) * .speed) / 1000000 // Double divide to prevent overflow
        }
        var run_target_cycle = get_target_cycle(time_now)
        // Sleep any ahead of time away
        while .cycle > Int run_target_cycle && get_command(ctl_command_id) == .last_command_id {
            sleep(1.ms())
            .cycle -= 1
            mode_refresh()
            .cycle += 1
            run_target_cycle = get_target_cycle(get_time())
        }
    }
}
def mode_refresh() {
    // Stub: check_output consumes .level_output (set in mode_run) directly;
    // #SIMULATION_STATE writes only feed the game's UI display, not the test result.
}
def set_error(input: @Type) {
    // No-op: error messages only feed the game's UI display, never the test
    // result. @Type accepts both the i18n tuple `(id, text)` (used by test.si
    // check_outputs) and a plain String.
}
def reset_sim() {
    .time_component_last = U64 0
    set_error("")
    .global_seed = Seed (get_command(ctl_test) + 1) // 0 can't be seed
    .last_level_manual_input = 0
    set_setting(sim_cycle, U64 -1)
    set_setting(sim_short_circuit_component_id_1, U64 0)
    set_setting(sim_short_circuit_component_id_2, U64 0)
    set_setting(sim_test_result, U64 pass)
    .cycle = -1
    memory_clear(#SIMULATION_KEYBOARD_CHARACTER, 8192)
    memory_clear(#SIMULATION_KEYBOARD_COORDINATE, 8192)
    memory_clear(#SIMULATION_STATE, 268)
}
type ComponentType Enum[com_off, com_on, com_not_bit, com_and_bit, com_and_3_bit, com_nand_bit, com_or_bit, com_or_3_bit, com_nor_bit, com_xor_bit, com_xnor_bit, com_switch_bit, com_delay_line_bit, com_register_bit, com_full_adder, com_maker_bit_8, com_splitter_bit_8, com_not_word, com_or_word, com_and_word, com_nand_word, com_nor_word, com_xor_word, com_xnor_word, com_switch_word, com_equal, com_less_u, com_less_s, com_neg, com_add, com_mul, com_div, com_lsl, com_lsr, com_rol, com_ror, com_asr, com_counter, com_register_word, com_level_output_8_pin, com_mux, com_decoder_1, com_decoder_2, com_decoder_3, com_constant, com_splitter_word_2, com_maker_word_2, com_clz, com_register_word_config, com_delay_line_word_asm, com_load_port, com_delay_line_word, com_store_port, com_ctz, com_cc_level_output, com_level_gate, com_level_input_1_pin, com_level_input_word, com_level_input_switched, com_level_input_2_pin, com_level_input_3_pin, com_level_input_4_pin, com_level_output_1_pin, com_level_output_word, com_level_output_switched, com_level_output_2_pin, com_level_output_3_pin, com_level_output_4_pin, com_level_output_counter, com_custom, com_cc_input, com_cc_output, com_probe_memory_bit, com_probe_memory_word, com_probe_wire_bit, com_probe_wire_word, com_halt, com_segment_display, com_static_value, com_screen, com_time, com_keyboard, com_static_eval, com_verilog_input, com_verilog_output, com_maker_word_4, com_maker_word_8, com_splitter_word_4, com_splitter_word_8, com_static_indexer, com_inc, com_cc_level_input, com_mod, com_splitter_bit_2, com_splitter_bit_4, com_maker_bit_2, com_maker_bit_4, com_concatenator_2, com_concatenator_4, com_concatenator_8, com_static_indexer_config, com_ram, com_delay_line_word_config]
def get_tick() Int { return .cycle }
def get_test() Int { return Int get_command(ctl_test) }
def get_gate_score() Int { return __GATE_SCORE__}
def get_delay_score() Int { return 0 }
def get_component_count() Int { return __COMPONENT_COUNT__ }
def get_component_count(component_type: ComponentType) Int {
    const #COUNTS = [__COUNTS__]
    return #COUNTS[Int component_type]
}
def ui_set_position(id: String, x: Int, y: Int) {
    switch id
        case "table" { store(.ui_buffer + 8, U64 (x << 16 | (y & 0xffff))) }
}
def ui_set_width(id: String, value: Int) {
    switch id
}
def ui_set_hidden(id: String, value: Bool) {
    switch id
        case "table[0][0]" { store(.ui_buffer + 88, U64 value) }
        case "table[0][1]" { store(.ui_buffer + 96, U64 value) }
        case "table[1][0]" { store(.ui_buffer + 104, U64 value) }
        case "table[1][1]" { store(.ui_buffer + 112, U64 value) }
        case "table[2][0]" { store(.ui_buffer + 120, U64 value) }
        case "table[2][1]" { store(.ui_buffer + 128, U64 value) }
        case "table[3][0]" { store(.ui_buffer + 136, U64 value) }
        case "table[3][1]" { store(.ui_buffer + 144, U64 value) }
        case "table[4][0]" { store(.ui_buffer + 152, U64 value) }
        case "table[4][1]" { store(.ui_buffer + 160, U64 value) }
        case "table" { store(.ui_buffer + 0, U64 value) }
}
def ui_set_size(id: String, value: Int) {
    switch id
        case "table" { store(.ui_buffer + 40, U64 value) }
}
def ui_set_column_header_size(id: String, value: Int) {
    switch id
}
def ui_set_row_header_size(id: String, value: Int) {
    switch id
        case "table" { store(.ui_buffer + 56, U64 value) }
}
def ui_set_color(id: String, value: Int) {
    switch id
        case "table" { store(.ui_buffer + 48, U64 value) }
}
def ui_set_address_color(id: String, value: Int) {
    switch id
}
def ui_set_instruction_color(id: String, value: Int) {
    switch id
}
def ui_set_description_color(id: String, value: Int) {
    switch id
}
def ui_set_column_header_color(id: String, value: Int) {
    switch id
}
def ui_set_row_header_color(id: String, value: Int) {
    switch id
        case "table" { store(.ui_buffer + 64, U64 value) }
}
def ui_set_text(id: String, text: String) {
    switch id
        case "table[0][0]" { set_text(text, 10000) }
        case "table[0][1]" { set_text(text, 20000) }
        case "table[1][0]" { set_text(text, 30000) }
        case "table[1][1]" { set_text(text, 40000) }
        case "table[2][0]" { set_text(text, 50000) }
        case "table[2][1]" { set_text(text, 60000) }
        case "table[3][0]" { set_text(text, 70000) }
        case "table[3][1]" { set_text(text, 80000) }
        case "table[4][0]" { set_text(text, 90000) }
        case "table[4][1]" { set_text(text, 100000) }
}
def ui_set_image_name(id: String, text: String) {
    switch id
}
def ui_set_instruction(index: Int, code: Int, pointer: Int) {
    switch index
}
def add_keyboard_value(key_down: Bool, key: U8) {
    let value = U16 key_down | (U16 key) << 8
    var index = load(<U16>, #SIMULATION_KEYBOARD_CHARACTER + 4098)
    store(#SIMULATION_KEYBOARD_CHARACTER + index, value)
    store(#SIMULATION_KEYBOARD_CHARACTER + 4098, (index + 8) % 4096)
}
def get_last_time() Int { return Int .time_component_last }
def get_screen_connection(screen_name: String) String {
}
def get_memory(label: String) Int {
}
def get_load_port_input(label: String) Int {
}
def get_screen_settings(label: String, index: Int) Int {
}
def get_output(user_label: String, index: Int) Int {
}
def get_ram_value(label: String, address: Int, size: @Size) @Size {
}
def set_ram_value(label: String, offset: Int, value: @Type) {
}
var last_level_manual_input = U64 0
var time_component_last = U64 0
var manual_input_seen = false
run_sim: while true {
    reset_sim()
    while true {
        mode_refresh()
        #if in_scope(on_ui_update){
            on_ui_update(.level_input, .level_output)
        }
        while get_command(ctl_command_id) == last_command_id {
            #if in_scope(on_manual_input){
                if last_level_manual_input != get_command(ctl_level_manual_input_id) {
                    last_level_manual_input = get_command(ctl_level_manual_input_id)
                    on_manual_input(Int get_command(ctl_level_manual_input))
                    manual_input_seen = true
                }
            }
            sleep(1.ms())
        }
        if manual_input_seen {
            manual_input_seen = false
            break
        }
        last_command_id = get_command(ctl_command_id) // Read this before the command in case of race condition
        set_setting(sim_last_command_id, last_command_id)
        var command = SimCommand get_command(ctl_command)
        switch command
            case run {
                var target_cycle = Int get_setting(sim_target_cycle)
                set_setting(sim_running, U64 1)
                mode_run(target_cycle)
                set_setting(sim_running, U64 0)
            }
            case refresh {}
            case mode_reset {
                var i = 0
                while i < 1024 {
                    input_replay[i] = 0
                    i += 1
                }
                break
            }
            case quit_simulation {
                break run_sim // We need to break out instead of thread_exit() to make sure everything is cleaned up
            }
    }
}"#;

/// ComponentType enum length (indices 0..=100 used by `#COUNTS`).
const COUNTS_LEN: usize = 101;

fn counts_array(circuit: &Circuit) -> Result<String, String> {
    let mut counts = vec![0u64; COUNTS_LEN];
    for comp in &circuit.components {
        if is_level_output(comp.kind) {
            continue; // match the game: outputs aren't counted in #COUNTS.
        }
        let idx = kind_to_enum(comp.kind).ok_or(format!("UNSUPPORTED_KIND|{}", comp.kind))?;
        counts[idx] += 1;
    }
    Ok(counts
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(", "))
}

/// Number of gate (non-I/O) components, for `get_gate_score` / component count.
fn gate_components(circuit: &Circuit) -> usize {
    circuit
        .components
        .iter()
        .filter(|c| !is_level_input(c.kind) && !is_level_output(c.kind))
        .count()
}

/// Generate a full current-dialect DSL for `circuit` under `tpl`.
/// Output field bit-width comes from the circuit's level-output component, not
/// from test.si (test.si's field type is best-effort: the `#CORRECT_OUTPUT`
/// element type, else U1). kind 68 (1-pin) is always U1; kind 69 (word) is
/// `U{word_size}`. Overrides the template's types and rebuilds the struct.
///
/// Output components split into two kinds:
/// - value outputs (kind 68/69/70/73/74/75/77) — one per `tpl.output_fields`
/// - z-flag outputs (kind 112) — one per `tpl.output_z_fields`
///
/// A level may have z-flag fields in `check_output` but no z probe in the
/// circuit; in that case the field is left at the struct default (false).
fn correct_output_types(mut tpl: LevelTemplate, circuit: &Circuit) -> Result<LevelTemplate, String> {
    let mut value_comps: Vec<&Component> = circuit
        .components
        .iter()
        .filter(|c| is_level_output(c.kind) && c.kind != 112)
        .collect();
    value_comps.sort_by_key(|c| (c.word_size, c.position.0, c.position.1));
    let z_comps: Vec<&Component> = circuit
        .components
        .iter()
        .filter(|c| c.kind == 112)
        .collect();
    if value_comps.len() != tpl.output_fields.len() {
        return Err(format!(
            "OUTPUT_FIELD_COUNT_MISMATCH|value circuit={} tpl={}",
            value_comps.len(),
            tpl.output_fields.len()
        ));
    }
    if z_comps.len() > tpl.output_z_fields.len() {
        return Err(format!(
            "OUTPUT_FIELD_COUNT_MISMATCH|z circuit={} tpl={}",
            z_comps.len(),
            tpl.output_z_fields.len()
        ));
    }
    for (comp, field) in value_comps.iter().zip(tpl.output_fields.iter_mut()) {
        let t = if comp.kind == 68 {
            "U1".to_string()
        } else {
            format!("U{}", comp.word_size.max(1))
        };
        field.1 = t;
    }
    let z = tpl.output_z_fields.clone();
    tpl.output_struct = crate::dll::test_si::build_output_struct(&tpl.output_fields, &z);
    Ok(tpl)
}

pub fn generate(circuit: &Circuit, tpl: &LevelTemplate) -> Result<String, String> {
    let tpl = correct_output_types(tpl.clone(), circuit)?;
    let conn = pins::resolve(circuit)?;
    let sim = emit_circuit_sim(circuit, &conn, &tpl)?;
    let counts = counts_array(circuit)?;
    let gates = gate_components(circuit);

    let structs = format!("{}\n{}", tpl.input_struct, tpl.output_struct);
    let out = SKELETON
        .replace("__INPUT_OUTPUT_STRUCTS__", &structs)
        .replace("__TEST_DEFS__", &tpl.test_defs)
        .replace("__CIRCUIT_SIM__", &sim)
        .replace("__GATE_SCORE__", &gates.to_string())
        .replace("__COMPONENT_COUNT__", &circuit.components.len().to_string())
        .replace("__COUNTS__", &counts);

    if out.contains("\n\n") {
        return Err("GENERATED_DIALECT_VIOLATION|blank line".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dll::test_si;

    fn and_gate_circuit() -> Circuit {
        let payload = std::fs::read("../sim-shim/fixtures/and_gate.data")
            .expect("read circuit fixture");
        crate::circuit::codec::decode_circuit(&payload).expect("decode circuit")
    }

    fn and_gate_template() -> LevelTemplate {
        let content = std::fs::read_to_string("../sim-shim/fixtures/test_si/and_gate.si")
            .expect("read test.si fixture");
        test_si::parse(&content, "and_gate").expect("parse test.si")
    }

    #[test]
    #[ignore = "needs the player's save directory; run via --ignored"]
    fn generated_and_gate_dsl_matches_spike_structure() {
        let circuit = and_gate_circuit();
        let dsl = generate(&circuit, &and_gate_template()).expect("generate");
        // Input component (kind 63, 2-pin) reads the packed `input: U2` field.
        assert!(
            dsl.contains("var vid0 = U1 (.level_input.input >> 0 & 1)"),
            "pin0 must read bit0:\n{dsl}"
        );
        assert!(
            dsl.contains("var vid1 = U1 (.level_input.input >> 1 & 1)"),
            "pin1 must read bit1"
        );
        // The generated circuit-sim must wire Input → nand → not → Output.
        assert!(
            dsl.contains("var vid2 = U1 (U1 ~((U1 vid0) & (U1 vid1)))"),
            "nand must AND vid0/vid1:\n{dsl}"
        );
        assert!(
            dsl.contains("var vid3 = U1 (U1 ~(U1 vid2))"),
            "not must invert nand"
        );
        assert!(
            dsl.contains(".level_output.output = U1 vid3"),
            "output must read the not gate"
        );
        assert!(
            dsl.contains(".level_output.output_is_z = false"),
            "z-flag must be cleared"
        );
        // Compact dialect: no blank lines anywhere.
        assert!(!dsl.contains("\n\n"), "no blank lines in generated DSL");
    }

    #[test]
    #[ignore = "needs the player's save directory; run via --ignored"]
    fn generated_dsl_has_no_leftover_markers() {
        let circuit = and_gate_circuit();
        let dsl = generate(&circuit, &and_gate_template()).expect("generate");
        assert!(!dsl.contains("__"), "no unfilled markers remain");
    }
}

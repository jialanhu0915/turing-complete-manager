//! Per-level DSL test templates for the PoC levels.
//!
//! A template captures everything about a level's test contract that the
//! generator cannot derive from `circuit.data`: the Input/Output struct shapes,
//! how `get_input` produces test vectors, and how `check_output` evaluates the
//! circuit's output against the level's expected function. The generated DSL
//! for each of these was verified end-to-end in Phase 1 (and_gate) and Phase 4
//! (or_gate / not_gate).
//!
//! The `get_input` functions cycle the full input truth table deterministically
//! (`a = tick&1`, `b = (tick>>1)&1`) — every combination is covered within a
//! handful of cycles, which is all the exec harness needs to discriminate a
//! correct circuit from a broken one.

use super::gen::LevelTemplate;

pub const AND_GATE: LevelTemplate = LevelTemplate {
    input_struct: "type Input {\n    a: U1,\n    b: U1,\n}",
    output_struct: "type Output {\n    result: U1,\n    result_is_z: Bool,\n}",
    get_input: "def get_input(tick: Int) Input {\n    var a = U1 (tick & 1)\n    var b = U1 ((tick >> 1) & 1)\n    return Input {a: U1 a, b: U1 b}\n}",
    check_output: "def check_output(tick: Int, input: Input, output: Output) TestResult {\n    var expected = false\n    if (U64 input.a & U64 input.b) != 0 {\n        expected = true\n    }\n    if expected {\n        if output.result == 0 {\n            return fail\n        }\n    } else {\n        if output.result != 0 {\n            return fail\n        }\n    }\n    if tick == 2047 {\n        return win\n    }\n    return pass\n}",
    input_fields: &[("a", "U1"), ("b", "U1")],
    output_fields: &[("result", "U1")],
    result_is_z: true,
};

pub const OR_GATE: LevelTemplate = LevelTemplate {
    input_struct: "type Input {\n    a: U1,\n    b: U1,\n}",
    output_struct: "type Output {\n    result: U1,\n    result_is_z: Bool,\n}",
    get_input: "def get_input(tick: Int) Input {\n    var a = U1 (tick & 1)\n    var b = U1 ((tick >> 1) & 1)\n    return Input {a: U1 a, b: U1 b}\n}",
    check_output: "def check_output(tick: Int, input: Input, output: Output) TestResult {\n    var expected = false\n    if (U64 input.a | U64 input.b) != 0 {\n        expected = true\n    }\n    if expected {\n        if output.result == 0 {\n            return fail\n        }\n    } else {\n        if output.result != 0 {\n            return fail\n        }\n    }\n    if tick == 2047 {\n        return win\n    }\n    return pass\n}",
    input_fields: &[("a", "U1"), ("b", "U1")],
    output_fields: &[("result", "U1")],
    result_is_z: true,
};

pub const NOT_GATE: LevelTemplate = LevelTemplate {
    input_struct: "type Input {\n    a: U1,\n}",
    output_struct: "type Output {\n    result: U1,\n    result_is_z: Bool,\n}",
    get_input: "def get_input(tick: Int) Input {\n    var a = U1 (tick & 1)\n    return Input {a: U1 a}\n}",
    check_output: "def check_output(tick: Int, input: Input, output: Output) TestResult {\n    var expected = false\n    if (U64 input.a) == 0 {\n        expected = true\n    }\n    if expected {\n        if output.result == 0 {\n            return fail\n        }\n    } else {\n        if output.result != 0 {\n            return fail\n        }\n    }\n    if tick == 2047 {\n        return win\n    }\n    return pass\n}",
    input_fields: &[("a", "U1")],
    output_fields: &[("result", "U1")],
    result_is_z: true,
};

/// Look up a PoC level's template by `level_id`.
pub fn template_for_level(level_id: &str) -> Option<&'static LevelTemplate> {
    match level_id {
        "and_gate" => Some(&AND_GATE),
        "or_gate" => Some(&OR_GATE),
        "not_gate" => Some(&NOT_GATE),
        _ => None,
    }
}

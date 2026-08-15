// Minimal CURRENT-dialect DSL that compiles successfully through compile.dll.
// Proves the shim + game JIT compiler pipeline works end to end.
//
// Key grammar facts (reverse-engineered 2026-08-08):
//   - `switch expr` with indented `case {}` blocks (NOT standalone statements)
//   - `var x = <value>` required (no `var x: Type` without initializer)
//   - top-level forward refs OK (mode_run calls helper defined later)
//   - same-function nested forward refs OK
//   - cross-function nested def calls FAIL in current compiler
//     (replay.nim's old dialect relies on these and will NOT compile)
type SimCommand Enum[run, refresh, mode_reset, quit_simulation]
type CommandIndex Enum[ctl_command, ctl_command_id, ctl_tick_speed_ms, ctl_exit, ctl_level_manual_input, ctl_level_manual_input_id, ctl_test]
type StateIndex Enum[sim_address, sim_cycle, sim_target_cycle, sim_test_result, sim_last_command_id, sim_error_component, sim_short_circuit_component_id_1, sim_short_circuit_component_id_2, sim_short_circuit_pin_1, sim_short_circuit_pin_2, sim_short_circuit_top_level_permanent_id_1, sim_short_circuit_top_level_permanent_id_2, sim_short_circuit_value_1, sim_short_circuit_value_2, sim_short_circuit_any_top_level_wire_id, sim_running]
type TestResult Enum[pass, win, fail]

def mode_run() {
    var x = helper(1)
}

def helper(tick: Int) Int {
    return tick + 1
}

run_sim: while true {
    var command = SimCommand 0
    switch command
        quit_simulation { break run_sim }
        run { mode_run() }
        refresh {}
        mode_reset {}
}

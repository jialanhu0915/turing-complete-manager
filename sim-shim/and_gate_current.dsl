// and_gate DSL converted to CURRENT compile.dll dialect (see convert_dialect.py).
type SimCommand Enum[run, refresh, mode_reset, quit_simulation]
type CommandIndex Enum[ctl_command, ctl_command_id, ctl_cycle_speed_ms, ctl_exit, ctl_level_manual_input, ctl_level_manual_input_id, ctl_test]
type StateIndex Enum[sim_address, sim_cycle, sim_target_cycle, sim_test_result, sim_last_command_id, sim_error_component, sim_short_circuit_component_id_1, sim_short_circuit_component_id_2, sim_short_circuit_pin_1, sim_short_circuit_pin_2, sim_short_circuit_top_level_permanent_id_1, sim_short_circuit_top_level_permanent_id_2, sim_short_circuit_value_1, sim_short_circuit_value_2, sim_short_circuit_any_top_level_wire_id, sim_running]
type TestResult Enum[pass, win, fail]
type Input {
    condition: U3,
    input: U8,
}
type Output {
    result: U1,
    result_is_z: Bool,
}
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
def get_input(tick: Int) Input {
    var x = tick
    x ^= x << 1
    x ^= x >> 5
    x ^= x << 8
    var condition = x & 0b111
    var value = (x >> 3) & 0xff
    if value & 0b10000000 != 0 {
        value = value ^ 0xff
        value += 1
        value = -value
    }
    var condition_text = " "
    if (condition / 4) % 2 == 0 {
        condition_text += "[OFF]"
    } else {
        condition_text += "[ON]"
    }
    if (condition / 2) % 2 == 0 {
        condition_text += "[OFF]"
    } else {
        condition_text += "[ON]"
    }
    if (condition / 1) % 2 == 0 {
        condition_text += "[OFF]"
    } else {
        condition_text += "[ON]"
    }
    condition_text += " "
    ui_set_text("table[1][1]", `{value}`)
    switch condition
        0 {
            ui_set_text("table[2][1]", `NEVER`)
            ui_set_text("table[3][1]", "[OFF]")
        }
        1 {
            ui_set_text("table[2][1]", `ALWAYS`)
            ui_set_text("table[3][1]", "[ON]")
        }
        2 {
            ui_set_text("table[2][1]", `{value} = 0`)
            if value == 0 {
                ui_set_text("table[3][1]", "[ON]")
            } else {
                ui_set_text("table[3][1]", "[OFF]")
            }
        }
        3 {
            ui_set_text("table[2][1]", `{value} ≠ 0`)
            if value == 0 {
                ui_set_text("table[3][1]", "[OFF]")
            } else {
                ui_set_text("table[3][1]", "[ON]")
            }
        }
        4 {
            ui_set_text("table[2][1]", `{value} < 0`)
            if value < 0 {
                ui_set_text("table[3][1]", "[ON]")
            } else {
                ui_set_text("table[3][1]", "[OFF]")
            }
        }
        5 {
            ui_set_text("table[2][1]", `{value} ≥ 0`)
            if value < 0 {
                ui_set_text("table[3][1]", "[OFF]")
            } else {
                ui_set_text("table[3][1]", "[ON]")
            }
        }
        6 {
            ui_set_text("table[2][1]", `{value} ≤ 0`)
            if value <= 0 {
                ui_set_text("table[3][1]", "[ON]")
            } else {
                ui_set_text("table[3][1]", "[OFF]")
            }
        }
        7 {
            ui_set_text("table[2][1]", `{value} > 0`)
            if value <= 0 {
                ui_set_text("table[3][1]", "[OFF]")
            } else {
                ui_set_text("table[3][1]", "[ON]")
            }
        }
    ui_set_text("table[0][1]", condition_text)
    return Input {condition: U3 condition, input: U8 value}
}
def check_output(tick: Int, input: Input, output: Output) TestResult {
    if output.result_is_z {
        ui_set_text("table[4][1]", "[ANY]")
    } elif output.result == 0 {
        ui_set_text("table[4][1]", "[OFF]")
    } else {
        ui_set_text("table[4][1]", "[ON]")
    }
    var expected = false
    var condition = input.condition
    var value = asr(Int input.input << 56, 56)
    switch condition
        0 {
            expected = false
        }
        1 {
            expected = true
        }
        2 {
            expected = value == 0
        }
        3 {
            expected = value != 0
        }
        4 {
            expected = value < 0
        }
        5 {
            expected = value >= 0
        }
        6 {
            expected = value <= 0
        }
        7 {
            expected = value > 0
        }
    if expected {
        if output.result == 0 {
            return fail
        }
    } else {
        if output.result != 0 {
            return fail
        }
    }
    if tick == 2047 {
        return win
    }
    return pass
}
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
                // NOTE: cannot use max(get_setting(sim_test_result), res) here —
                // settings[sim_test_result] aliases input_replay[1], which holds
                // the last input value (e.g. 96), so max(96, fail=2) = 96. Write
                // the failure directly (a fail is terminal for a single run).
                set_setting(sim_test_result, U64 res)
            }
        }
        while .cycle < burst_target_cycle {
            .level_input = get_input(.cycle + 1)
            // 1 com_cc_level_input 3 8886406332321803229 Condition
            var vid256 = U3 .level_input.condition
            .input_replay[0] = U64 .level_input.condition
            // 3 com_cc_level_input 8 3888339095491657582 Input
            var vid258 = U8 .level_input.input
            .input_replay[1] = U64 .level_input.input
            // 4 com_decoder_3 2 4144550276953431113
            var val4 = (((((U1 0x0)) & 1) == 1) ? 8 : U8 ((U3 vid256) & 0b111))
            var vid259 = U1 (U64 (val4 == 0))
            var vid260 = U1 (U64 (val4 == 1))
            var vid261 = U1 (U64 (val4 == 2))
            var vid262 = U1 (U64 (val4 == 3))
            var vid263 = U1 (U64 (val4 == 4))
            var vid264 = U1 (U64 (val4 == 5))
            var vid265 = U1 (U64 (val4 == 6))
            var vid266 = U1 (U64 (val4 == 7))
            // 0 com_none 2 0
            // 2 com_cc_level_output 1 412430996129987082 Result
            .level_output.result = U1 ((U1 0x0))
            .level_output.result_is_z = true
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
    // 1 com_cc_level_input 3 8886406332321803229 Condition
    let value_id256 = .input_replay[0]
    store(#SIMULATION_STATE + 256, U3 (value_id256))
    store(#SIMULATION_STATE + 257, U3 (value_id256))
    // 3 com_cc_level_input 8 3888339095491657582 Input
    let value_id258 = .input_replay[1]
    store(#SIMULATION_STATE + 258, U8 (value_id258))
    // 4 com_decoder_3 2 4144550276953431113
    var val4 = (((((U1 0x0)) & 1) == 1) ? 8 : U8 (load(<U3>, #SIMULATION_STATE + 256) & 0b111))
    let value_id259 = (U64 (val4 == 0))
    store(#SIMULATION_STATE + 259, U1 (value_id259))
    let value_id260 = (U64 (val4 == 1))
    store(#SIMULATION_STATE + 260, U1 (value_id260))
    let value_id261 = (U64 (val4 == 2))
    store(#SIMULATION_STATE + 261, U1 (value_id261))
    let value_id262 = (U64 (val4 == 3))
    store(#SIMULATION_STATE + 262, U1 (value_id262))
    let value_id263 = (U64 (val4 == 4))
    store(#SIMULATION_STATE + 263, U1 (value_id263))
    let value_id264 = (U64 (val4 == 5))
    store(#SIMULATION_STATE + 264, U1 (value_id264))
    let value_id265 = (U64 (val4 == 6))
    store(#SIMULATION_STATE + 265, U1 (value_id265))
    let value_id266 = (U64 (val4 == 7))
    store(#SIMULATION_STATE + 266, U1 (value_id266))
    // 0 com_none 2 0
    // 2 com_cc_level_output 1 412430996129987082 Result
}
def set_error(input: String) {
    let len = input.len()
    store(.error_buffer, U16 len)
    var i = 0
    while i < len {
        store(.error_buffer + i + 2, U8 input[i])
        i += 1
    }
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
def get_gate_score() Int { return 28}
def get_delay_score() Int { return 0 }
def get_component_count() Int { return 4 }
def get_component_count(component_type: ComponentType) Int {
    const #COUNTS = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    return #COUNTS[Int component_type]
}
def ui_set_position(id: String, x: Int, y: Int) {
    switch id
        "table" { store(.ui_buffer + 8, U64 (x << 16 | (y & 0xffff))) }
}
def ui_set_width(id: String, value: Int) {
    switch id
}
def ui_set_hidden(id: String, value: Bool) {
    switch id
        "table[0][0]" { store(.ui_buffer + 88, U64 value) }
        "table[0][1]" { store(.ui_buffer + 96, U64 value) }
        "table[1][0]" { store(.ui_buffer + 104, U64 value) }
        "table[1][1]" { store(.ui_buffer + 112, U64 value) }
        "table[2][0]" { store(.ui_buffer + 120, U64 value) }
        "table[2][1]" { store(.ui_buffer + 128, U64 value) }
        "table[3][0]" { store(.ui_buffer + 136, U64 value) }
        "table[3][1]" { store(.ui_buffer + 144, U64 value) }
        "table[4][0]" { store(.ui_buffer + 152, U64 value) }
        "table[4][1]" { store(.ui_buffer + 160, U64 value) }
        "table" { store(.ui_buffer + 0, U64 value) }
}
def ui_set_size(id: String, value: Int) {
    switch id
        "table" { store(.ui_buffer + 40, U64 value) }
}
def ui_set_column_header_size(id: String, value: Int) {
    switch id
}
def ui_set_row_header_size(id: String, value: Int) {
    switch id
        "table" { store(.ui_buffer + 56, U64 value) }
}
def ui_set_color(id: String, value: Int) {
    switch id
        "table" { store(.ui_buffer + 48, U64 value) }
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
        "table" { store(.ui_buffer + 64, U64 value) }
}
def ui_set_text(id: String, text: String) {
    switch id
        "table[0][0]" { set_text(text, 10000) }
        "table[0][1]" { set_text(text, 20000) }
        "table[1][0]" { set_text(text, 30000) }
        "table[1][1]" { set_text(text, 40000) }
        "table[2][0]" { set_text(text, 50000) }
        "table[2][1]" { set_text(text, 60000) }
        "table[3][0]" { set_text(text, 70000) }
        "table[3][1]" { set_text(text, 80000) }
        "table[4][0]" { set_text(text, 90000) }
        "table[4][1]" { set_text(text, 100000) }
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
    if user_label == "Condition" && index == 0 { return Int (U64 load(<U3>, #SIMULATION_STATE + 1)) }
    if user_label == "Input" && index == 0 { return Int (U64 load(<U8>, #SIMULATION_STATE + 1)) }
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
            run {
                var target_cycle = Int get_setting(sim_target_cycle)
                set_setting(sim_running, U64 1)
                mode_run(target_cycle)
                set_setting(sim_running, U64 0)
            }
            refresh {}
            mode_reset {
                var i = 0
                while i < 1024 {
                    input_replay[i] = 0
                    i += 1
                }
                break
            }
            quit_simulation {
                break run_sim // We need to break out instead of thread_exit() to make sure everything is cleaned up
            }
    }
}
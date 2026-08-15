//! `and_gate` 起步示例 —— 走一遍 SDK 的三层 API。
//!
//! ① 数据格式    ：解码 `and_gate` 的 `circuit.data`（v15），再编码回环验证
//! ② 编译执行    ：从 `test.si` 解析关卡模板，生成 DSL
//! ③ 仿真运行时  ：通过 shim.dll 编译 + JIT 执行，读取测试结果
//!
//! 运行（无需游戏本体，走 ①②；③ 在缺 shim.dll 时优雅跳过）：
//!   cargo run --example and_gate
//!
//! 第 ③ 步需要本机装有《Turing Complete》游戏本体（compile.dll）并已构建
//! `sim-shim/shim.dll`（见 `sim-shim/build.bat`）。

use std::path::PathBuf;

use tc_mod_sdk::circuit::codec;
use tc_mod_sdk::dll::{gen, runtime, test_si};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/fixtures");

    // ── ① 数据格式：解码 + 编码回环 ──────────────────────────────────────
    let data = std::fs::read(fixtures.join("and_gate.data"))?;
    let circuit = codec::decode_circuit(&data)?;
    println!(
        "[①] 解码 circuit.data: {} 组件, {} 连线",
        circuit.components.len(),
        circuit.wires.len()
    );

    let re_encoded = codec::encode_v15(&circuit)?;
    let re_decoded = codec::decode_circuit(&re_encoded)?;
    assert_eq!(re_decoded, circuit, "v15 编码回环失败");
    println!("[①] v15 编码回环 OK（{} bytes）", re_encoded.len());

    // ── ② 编译执行：解析 test.si → 生成 DSL ──────────────────────────────
    let si = std::fs::read_to_string(fixtures.join("and_gate.si"))?;
    let tpl = test_si::parse(&si, "and_gate")?;
    let dsl = gen::generate(&circuit, &tpl)?;
    println!("[②] 生成 DSL: {} 行", dsl.lines().count());

    // ── ③ 仿真运行时：编译 + 执行（需游戏本体 + shim.dll）────────────────
    // test_number = 0（第一个用例），target_cycle = 2050（覆盖 test.si 的
    // 真值表周期）。
    match runtime::run_circuit_test("and_gate", "default", &circuit, &dsl, 0, 2050) {
        Ok(report) => {
            println!(
                "[③] 编译 {}",
                if report.compiled_ok { "OK" } else { "失败" }
            );
            if let Some(r) = report.test_result {
                // 0 = pass, 1 = win, 2 = fail
                println!(
                    "[③] 测试结果: {}（0=pass, 1=win, 2=fail），运行 {} 周期",
                    r, report.cycles_run
                );
            }
            if let Some(e) = &report.error {
                println!("[③] 错误: {e}");
            }
        }
        Err(e) => println!("[③] 跳过（{e}）"),
    }

    Ok(())
}

---
title: Simplex 语言参考
last_updated: 2026-08-10
scope: investigation
status: 初稿（基于 wiki 校对）
---

# Simplex 语言参考

> Wiki 来源：`turingcomplete.wiki/wiki/Custom_level_creation/Simplex`（CC BY-SA 4.0）。
> Wiki 自标注："This list is incomplete, and some of the function signatures are guesses."
> 本文是 wiki 校对归档，不替代官方文档。

`Simplex` 是 Turing Complete 自定义的编程语言，专用于在 `test.si` 文件里编写关卡测试逻辑（玩家电路的初始化与验证）。

**VSCode 语法插件**：[tc-si](https://marketplace.visualstudio.com/items?itemName=Michai.tc-si)（社区贡献）。

---

## 一、内置函数（按类别）

### 算术

| 函数 | 说明 |
|---|---|
| `log10(x: AnyInt) Int` | 以 10 为底对数 |
| `ulog2(x: AnyUInt) Int` | 无符号以 2 为底对数 |
| `log2(x: AnySInt) Int` | 有符号以 2 为底对数 |
| `asr(x: AnyInt, shift_amount: AnyInt) Int` | 算术右移 |
| `popcount(x: AnyInt) Int` | 1 的位数 |
| `trailing_zeros(x: AnyInt) Int` | 末尾 0 的位数 |

### 数组

| 函数 | 说明 |
|---|---|
| `array(element: @Type, length: Int) [@Type]` | 创建数组 |
| `type_of_element(array: [@Element]) Type` | 返回数组元素类型 |
| `quick_sort($arr: [@Any])` | 快速排序（in-place） |
| `sort($arr: [@Any])` | 排序 |

### 显示

| 函数 | 说明 |
|---|---|
| `hex(num: @Size) String` | 十六进制字符串 |
| `output(text: String)` | 打印到控制台（无换行） |
| `print(x: @Any)` | 打印到控制台（带换行） |
| `str(x: @Type) String` | 字符串表示 |

### 点函数（dot functions）

通过 `.` 运算符访问，如 `let arr = [U64 0, 0, 0]; print(arr.len()) // 3`。

| 函数 | 说明 |
|---|---|
| `high(a: [@Any]) Int` | 数组末尾元素 |
| `in(value: @Type, array: [@Type]) Bool` | 元素是否在数组中 |
| `len(array: [@Any]) Int` | 数组长度 |

### 内存（指针操作）

| 函数 | 说明 |
|---|---|
| `memory_clear(source: Ptr, length: Int)` | 清零内存区 |
| `memory_commit(pointer: Ptr, length: Int)` | 提交（OS 视角） |
| `memory_copy(source: Ptr, destination: Ptr, length: Int)` | 拷贝 |
| `memory_copy_reverse(source: Ptr, destination: Ptr, length: Int)` | 反向拷贝 |
| `memory_free(pointer: Ptr, size: Int)` | 释放 |
| `memory_reserve(length: Int) Ptr` | 预留（返回 Ptr） |

### 随机

| 函数 | 说明 |
|---|---|
| `random() Int` | 随机整数 |
| `random(max: Int) Int` | 0..max 范围随机整数 |
| `random(type: @Type) @Type` | 按类型随机值 |
| `randomize_seed(...)` | 重置种子（参数可变） |
| `sample(array: [@Any]) @Any` | 从数组抽一个 |

### 工具

| 函数 | 说明 |
|---|---|
| `assert(condition: Bool, error_code: Int)` | 断言（错误时设错误码） |
| `int_of_size(value: Int, size: Int): @Sxx` | 按位宽构造有符号整数（编译期常量）；例 `int_of_size(4, 32)` = `S32 4` |
| `sleep(duration: Time)` | 休眠 |

---

## 二、与 test.si 的关系

Simplex 是 `test.si` 文件的语言。`test.si` 提供关卡特定的 API（`check_output` / `get_input` / `arch_check_output` 等，详见 [`compile-signature.md`](compile-signature.md) §test.si API 校对），这些 API 加上 Simplex 的内置函数，组成完整的关卡测试语言。

`memory_*` 与 `random*` 系列与 `compile.dll` 编译产物的指针访问相关——`Ptr` 类型来自 `replay.nim` 的 `simulator_types`。

---

## 三、未覆盖项

Wiki 自标注的不完整性：

- 函数清单可能不全（部分内部函数未列出）
- 部分函数签名是"guesses"（wiki 团队尚未 100% 确认）
- **未覆盖**：类型系统完整规则、模块/导入机制、宏、异步语义

需要更完整定义时，应从 [`Stuffe/tc_campaign`](https://github.com/Stuffe/tc_campaign) 拉取真实 `test.si` 文件做样本分析。
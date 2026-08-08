#!/usr/bin/env python3
"""Extract the DSL `code:` string from replay.nim's first compile_and_run block.

The `code:` field is a Nim compile-time string concatenation of the form
`code: (lit) & $simulation_X & (lit) ...`. We parse the concatenation and
replace each `$simulation_X` with a placeholder address, producing the plain
DSL source that compile.dll sees.

Usage: python extract_dsl.py [block_index] [out_file]
  block_index: 0-based index of compile_and_run block (default 0)
  out_file:    output path (default and_gate.dsl)
"""
import re
import sys

REPLAY = r"E:\SteamLibrary\steamapps\common\Turing Complete\replay.nim"

# Placeholder addresses for the 9 simulation_* symbols (won't be dereferenced
# in a compile-only smoke test).
PLACEHOLDERS = {
    "simulation_commands":           "0x1000000",
    "simulation_settings":           "0x1000010",
    "simulation_input_replay":       "0x1000020",
    "simulation_output_history_pins": "0x1000030",
    "simulation_error_buffer":       "0x1000040",
    "simulation_ui_buffer":          "0x1000050",
    "simulation_state":              "0x1000060",
    "simulation_keyboard_character": "0x1000070",
    "simulation_keyboard_coordinate": "0x1000080",
}

def find_blocks(lines):
    """Return (start_line, end_line) of each code: triple-quote block."""
    blocks = []
    i = 0
    n = len(lines)
    while i < n:
        m = re.match(r'\s+code:\s*"""', lines[i])
        if m:
            start = i
            j = i
            while j < n and not lines[j].rstrip().endswith('"""))'):
                j += 1
            blocks.append((start, j))
            i = j + 1
        else:
            i += 1
    return blocks

def concat_eval(block_lines):
    """Evaluate the Nim concatenation, replacing $simulation_* with addresses.

    We scan the expression token-by-token so that `&` characters INSIDE string
    literals (e.g. the DSL's bitwise-AND operator) are not treated as concat
    separators.
    """
    # Strip leading `code: ` only. The trailing `"""))` is handled by the
    # scanner: the closing `"""` closes the final body literal, and `))` are
    # stray parens we ignore.
    text = "\n".join(block_lines)
    text = re.sub(r'^\s*code:\s*', '', text)

    out = []
    i = 0
    n = len(text)
    while i < n:
        c = text[i]
        if c in ' \t\r\n':
            i += 1
        elif c == '"':
            # A double-quoted string literal.
            if text.startswith('"""', i):
                end = text.find('"""', i + 3)
                if end < 0:
                    raise ValueError("unterminated triple-quote literal")
                lit = text[i + 3:end]
                i = end + 3
            else:
                # Regular "..." literal with escapes.
                j = i + 1
                buf = []
                while j < n:
                    if text[j] == '\\' and j + 1 < n:
                        buf.append(text[j:j + 2])
                        j += 2
                    elif text[j] == '"':
                        break
                    else:
                        buf.append(text[j])
                        j += 1
                lit = "".join(buf)
                i = j + 1
            lit = (lit.replace('\\n', '\n').replace('\\t', '\t')
                      .replace('\\"', '"').replace('\\\\', '\\'))
            out.append(lit)
        elif c == '$':
            # $simulation_X placeholder.
            m = re.match(r'\$(\w+)', text[i:])
            if not m:
                raise ValueError(f"bad placeholder at offset {i}")
            name = m.group(1)
            if name not in PLACEHOLDERS:
                raise ValueError(f"unknown placeholder ${name}")
            out.append(PLACEHOLDERS[name])
            i += m.end()
        elif c == '&':
            i += 1  # concat separator
        elif c == ')':
            i += 1  # trailing paren(s)
        else:
            raise ValueError(f"unexpected char {c!r} at offset {i} in {text[i:i+30]!r}")
    return "".join(out)

def main():
    block_idx = int(sys.argv[1]) if len(sys.argv) > 1 else 0
    out_file = sys.argv[2] if len(sys.argv) > 2 else "and_gate.dsl"

    with open(REPLAY, encoding="utf-8") as f:
        lines = f.readlines()

    blocks = find_blocks(lines)
    if block_idx >= len(blocks):
        print(f"ERROR: only {len(blocks)} blocks, requested {block_idx}", file=sys.stderr)
        sys.exit(1)

    start, end = blocks[block_idx]
    dsl = concat_eval(lines[start:end + 1])
    with open(out_file, "w", encoding="utf-8") as f:
        f.write(dsl)
    print(f"Wrote {out_file}: {len(dsl)} bytes, {dsl.count(chr(10))} lines")
    print(f"First 200 chars:\n{dsl[:200]}")

if __name__ == "__main__":
    main()

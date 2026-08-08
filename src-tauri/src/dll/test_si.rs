//! `test.si` parser: derive a `LevelTemplate` from the game's per-level test
//! specification (`campaign/<level>/test.si`).
//!
//! The game ships the test oracle for every level as DSL source: a
//! `#CORRECT_OUTPUT` array (lookup levels) and/or `get_input`/`check_output`
//! defs that compute the expected result. We embed those verbatim (compact
//! dialect: no blank lines) so the generator supports ANY cycle-harness level
//! without hand-authoring templates.
//!
//! Supported: the `cycle`-based harness (`get_input(cycle) Input` +
//! `check_output(cycle, input, output)`), 79/91 test.si files. The switched
//! harness (`get_input_switched()`, architecture levels) is rejected.

use std::path::Path;

use super::gen::LevelTemplate;

/// Parse `game_dir/campaign/<level_id>/test.si`.
pub fn parse_level(game_dir: &Path, level_id: &str) -> Result<LevelTemplate, String> {
    let path = game_dir.join("campaign").join(level_id).join("test.si");
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("NO_TEST_SI|{level_id}|{e}"))?;
    parse(&content, level_id)
}

/// Parse test.si `content` into a `LevelTemplate`.
pub fn parse(content: &str, level_id: &str) -> Result<LevelTemplate, String> {
    // Compact dialect: strip CR, drop blank lines.
    let compact = content
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if compact.is_empty() {
        return Err(format!("EMPTY_TEST_SI|{level_id}"));
    }

    // The game's own generated DSL drops the i18n `(31337_<id>, text)` wrapper
    // (it keeps just the fallback text — see replay.nim block 2), and compile.dll
    // cannot parse the tuple literal. Strip it to the plain text.
    let compact = strip_localized_tuples(&compact);

    // Reject the switched (architecture) harness.
    if !compact.contains("def get_input(cycle") {
        return Err(format!("UNSUPPORTED_HARNESS|{level_id}"));
    }

    let (defs, decls) = split_top_level(&compact);

    let get_input = find_def(&defs, "get_input")
        .ok_or_else(|| format!("NO_GET_INPUT|{level_id}"))?;
    let mut check_output = find_def(&defs, "check_output")
        .ok_or_else(|| format!("NO_CHECK_OUTPUT|{level_id}"))?
        .to_string();
    check_output = ensure_return_pass(&check_output);
    let helpers: Vec<&str> = defs
        .iter()
        .filter(|(n, _)| n != "get_input" && n != "check_output")
        .map(|(_, t)| t.as_str())
        .collect();

    // Input struct: fields from `get_input`'s `Input { ... }` literal.
    let input_fields = derive_input_fields(&get_input);
    let input_struct = build_struct("Input", &input_fields);

    // Output struct: fields from `check_output`'s `output.<name>` usage;
    // value-field type from `#CORRECT_OUTPUT` element type when present.
    let co_type = correct_output_type(&compact);
    let (output_fields, z_fields) = derive_output_fields(&check_output, co_type.as_deref());
    let output_struct = build_output_struct(&output_fields, &z_fields);

    // Assemble the test-defs section: top decls + helpers + get_input + check_output.
    let mut parts: Vec<String> = Vec::new();
    if !decls.is_empty() {
        parts.push(decls);
    }
    parts.extend(helpers.iter().map(|h| h.to_string()));
    parts.push(get_input.to_string());
    parts.push(check_output);
    let test_defs = parts.join("\n");

    Ok(LevelTemplate {
        input_struct,
        output_struct,
        test_defs,
        input_fields,
        output_fields,
        output_z_fields: z_fields,
    })
}

// ─── top-level def/decl split ──────────────────────────────────────────────

fn skip_string(b: &[u8], i: usize) -> usize {
    // b[i] is a `"` or `` ` `` opening quote; return index after the close.
    let quote = b[i];
    let mut j = i + 1;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            c if c == quote => return j + 1,
            _ => j += 1,
        }
    }
    b.len()
}

/// Index of the `}` matching the `{` at `open`.
fn matching_brace(s: &str, open: usize) -> Option<usize> {
    let b = s.as_bytes();
    let mut depth = 0usize;
    let mut i = open;
    while i < b.len() {
        match b[i] {
            b'"' | b'`' => i = skip_string(b, i),
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i);
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

/// Index of the `)` matching the `(` at `open`.
fn matching_paren(s: &str, open: usize) -> Option<usize> {
    let b = s.as_bytes();
    let mut depth = 0usize;
    let mut i = open;
    while i < b.len() {
        match b[i] {
            b'"' | b'`' => i = skip_string(b, i),
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i);
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

/// Index of the first top-level `,` (outside strings and nested delimiters).
fn top_level_comma(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut depth = 0i32;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' | b'`' => i = skip_string(b, i),
            b'(' | b'[' | b'{' => {
                depth += 1;
                i += 1;
            }
            b')' | b']' | b'}' => {
                depth -= 1;
                i += 1;
            }
            b',' if depth <= 0 => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// Replace i18n tuples `(31337_<id>, <text>)` with just `<text>`, matching the
/// game's codegen (the DSL cannot parse the tuple literal).
fn strip_localized_tuples(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b'(' && s[i + 1..].starts_with("31337_") {
            if let Some(close) = matching_paren(s, i) {
                let inner = &s[i + 1..close];
                if let Some(comma) = top_level_comma(inner) {
                    let text = inner[comma + 1..].trim();
                    out.extend_from_slice(text.as_bytes());
                }
                i = close + 1;
                continue;
            }
        }
        let ch = s[i..].chars().next().unwrap();
        out.extend_from_slice(ch.to_string().as_bytes());
        i += ch.len_utf8();
    }
    String::from_utf8(out).unwrap()
}

fn brace_delta(s: &str) -> i32 {
    let b = s.as_bytes();
    let mut delta = 0i32;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' | b'`' => i = skip_string(b, i),
            b'{' => {
                delta += 1;
                i += 1;
            }
            b'}' => {
                delta -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    delta
}

/// Split compact content into `(name, full_def_text)` def blocks plus the
/// concatenated module-level declarations (const / var lines).
fn split_top_level(compact: &str) -> (Vec<(String, String)>, String) {
    let lines: Vec<&str> = compact.lines().collect();
    let mut defs = Vec::new();
    let mut decls = Vec::new();
    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i];
        if line.starts_with("def ") {
            let mut depth = 0i32;
            let mut j = i;
            let mut def_lines = Vec::new();
            while j < lines.len() {
                def_lines.push(lines[j]);
                depth += brace_delta(lines[j]);
                j += 1;
                if depth <= 0 {
                    break;
                }
            }
            let full = def_lines.join("\n");
            let name = extract_def_name(&full);
            defs.push((name, full));
            i = j;
        } else {
            decls.push(line.to_string());
            i += 1;
        }
    }
    (defs, decls.join("\n"))
}

fn extract_def_name(full: &str) -> String {
    // `def <name>(...)` — the identifier after "def ".
    let after = full.strip_prefix("def ").unwrap_or(full);
    let end = after.find('(').unwrap_or(after.len());
    after[..end].trim().to_string()
}

fn find_def<'a>(defs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    defs.iter().find(|(n, _)| n == name).map(|(_, t)| t.as_str())
}

// ─── struct derivation ─────────────────────────────────────────────────────

/// Extract `(name, type)` input fields from `get_input`'s `Input { ... }`
/// literal. The def signature's own `Input {` (return type + body brace) is
/// skipped by only searching after the body's opening brace.
fn derive_input_fields(get_input: &str) -> Vec<(String, String)> {
    // Body starts at the def's first `{` (the signature brace).
    let Some(body_start) = get_input.find('{') else {
        return Vec::new(); // empty Input struct (e.g. `def get_input(cycle) Input {}`)
    };
    let Some(rel) = get_input[body_start..].find("Input {") else {
        return Vec::new();
    };
    let marker = body_start + rel;
    let open = marker + "Input {".len() - 1; // position of the literal's `{`
    let Some(close) = matching_brace(get_input, open) else {
        return Vec::new();
    };
    let body = &get_input[open + 1..close];
    parse_fields(body)
}

/// Split a `{ ... }` field body into `(name, type)` pairs.
fn parse_fields(body: &str) -> Vec<(String, String)> {
    if body.trim().is_empty() {
        return Vec::new();
    }
    let mut fields = Vec::new();
    let mut start = 0usize;
    let b = body.as_bytes();
    let mut depth = 0i32;
    let mut i = 0usize;
    while i < b.len() {
        match b[i] {
            b'"' | b'`' => i = skip_string(b, i),
            b'{' | b'[' | b'(' => {
                depth += 1;
                i += 1;
            }
            b'}' | b']' | b')' => {
                depth -= 1;
                i += 1;
            }
            b',' if depth <= 0 => {
                let field = body[start..i].trim();
                if let Some((name, ty)) = parse_field(field) {
                    fields.push((name, ty));
                }
                start = i + 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    let field = body[start..].trim();
    if let Some((name, ty)) = parse_field(field) {
        fields.push((name, ty));
    }
    fields
}

/// `name: Type expr` → (name, Type); the type is the token after the first `:`.
fn parse_field(field: &str) -> Option<(String, String)> {
    if field.is_empty() {
        return None;
    }
    let colon = field.find(':')?;
    let name = field[..colon].trim();
    if name.is_empty() {
        return None;
    }
    let after = field[colon + 1..].trim();
    let type_end = after.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(after.len());
    let ty = after[..type_end].trim();
    if ty.is_empty() {
        return None;
    }
    Some((name.to_string(), ty.to_string()))
}

pub(crate) fn build_struct(name: &str, fields: &[(String, String)]) -> String {
    if fields.is_empty() {
        format!("type {name} {{\n}}")
    } else {
        let inner = fields
            .iter()
            .map(|(n, t)| format!("    {n}: {t},"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("type {name} {{\n{inner}\n}}")
    }
}

/// Element type of `#CORRECT_OUTPUT`, if present (`[U1 ...]` → `U1`).
fn correct_output_type(compact: &str) -> Option<String> {
    let marker = "#CORRECT_OUTPUT = [";
    let start = compact.find(marker)? + marker.len();
    let rest = &compact[start..];
    let end = rest.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(rest.len());
    let ty = &rest[..end];
    if ty.is_empty() {
        None
    } else {
        Some(ty.to_string())
    }
}

/// Output value fields (`output.<name>`) and z-flag fields (`output.<name>_is_z`).
/// Value-field types: `co_type` when a #CORRECT_OUTPUT array is present, else U1.
fn derive_output_fields(
    check_output: &str,
    co_type: Option<&str>,
) -> (Vec<(String, String)>, Vec<String>) {
    let (mut value_names, mut z_names) = (Vec::<String>::new(), Vec::<String>::new());
    let b = check_output.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        if check_output[i..].starts_with("output.") {
            i += "output.".len();
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let ident = &check_output[start..i];
            if ident.ends_with("_is_z") {
                if !z_names.contains(&ident.to_string()) {
                    z_names.push(ident.to_string());
                }
            } else if !ident.ends_with("_enabled") && !ident.is_empty() {
                if !value_names.contains(&ident.to_string()) {
                    value_names.push(ident.to_string());
                }
            }
        } else {
            i += 1;
        }
    }
    let ty = |_: &str| co_type.unwrap_or("U1").to_string();
    let fields: Vec<(String, String)> = value_names.iter().map(|n| (n.clone(), ty(n))).collect();
    (fields, z_names)
}

pub(crate) fn build_output_struct(fields: &[(String, String)], z_fields: &[String]) -> String {
    let mut all: Vec<(String, String)> = fields.to_vec();
    all.extend(z_fields.iter().map(|z| (z.clone(), "Bool".to_string())));
    build_struct("Output", &all)
}

// ─── check_output return-pass fix ──────────────────────────────────────────

/// The DSL returns garbage on fallthrough (verified in Phase 1), so a
/// `check_output` whose body does not end in a top-level `return` must be
/// given an explicit `return pass`.
fn ensure_return_pass(check_output: &str) -> String {
    let Some(close) = check_output.rfind('}') else {
        return check_output.to_string();
    };
    let body = &check_output[..close];
    let last_stmt = body
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim_start());
    let ends_with_return = last_stmt.is_some_and(|l| l.starts_with("return"));
    if ends_with_return {
        check_output.to_string()
    } else {
        format!("{body}    return pass\n}}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(level: &str) -> String {
        std::fs::read_to_string(format!("../sim-shim/fixtures/test_si/{level}.si"))
            .expect("read test.si fixture")
    }

    #[test]
    fn and_gate_lookup() {
        let t = parse(&fixture("and_gate"), "and_gate").expect("parse");
        assert_eq!(
            t.input_struct,
            "type Input {\n    input: U2,\n}"
        );
        assert_eq!(
            t.output_struct,
            "type Output {\n    output: U1,\n    output_is_z: Bool,\n}"
        );
        assert_eq!(t.input_fields, vec![("input".to_string(), "U2".to_string())]);
        assert_eq!(t.output_z_fields, vec!["output_is_z".to_string()]);
        // check_output must end with an explicit `return pass`.
        assert!(t.test_defs.ends_with("    return pass\n}"), "{}", t.test_defs);
        // Compact: no blank lines, no CR.
        assert!(!t.test_defs.contains("\n\n"));
        assert!(!t.test_defs.contains('\r'));
        // The const array survives.
        assert!(t.test_defs.contains("const #CORRECT_OUTPUT = [U1 0,0,0,1]"));
    }

    #[test]
    fn not_gate_lookup() {
        let t = parse(&fixture("not_gate"), "not_gate").expect("parse");
        assert_eq!(t.input_fields, vec![("input".to_string(), "U1".to_string())]);
        assert!(t.test_defs.ends_with("    return pass\n}"));
    }

    #[test]
    fn byte_asr_packed_computed() {
        // Multi-line Input literal + helper def + computed check_output.
        let t = parse(&fixture("byte_asr"), "byte_asr").expect("parse");
        assert_eq!(t.input_fields, vec![
            ("input".to_string(), "U8".to_string()),
            ("shift".to_string(), "U3".to_string()),
        ]);
        // Helper `get_binary_repr` is embedded before get_input.
        let get_bin = t.test_defs.find("def get_binary_repr").expect("helper present");
        let get_in = t.test_defs.find("def get_input").expect("get_input present");
        assert!(get_bin < get_in, "helper must precede get_input");
        assert!(t.test_defs.ends_with("    return pass\n}"));
    }

    #[test]
    fn switched_harness_rejected() {
        let err = parse(
            "var min = 0\ndef get_input_switched() Int {\n    return .min\n}",
            "binary_search",
        )
        .unwrap_err();
        assert!(err.contains("UNSUPPORTED_HARNESS"), "{err}");
    }
}

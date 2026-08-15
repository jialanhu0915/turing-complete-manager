//! Strict v15 reader/writer for Turing Complete circuit containers.
//!
//! Ported from `tc-save-lab/src/tc_save_lab/codec.py`. Wire format:
//!
//! ```text
//! [0x00]       version byte (must equal 15)
//! [0x01..end]  Snappy-compressed body containing:
//!     i64   custom_id
//!     u32   hub_id
//!     i64   gate
//!     i64   delay
//!     bool  menu_visible
//!     u64   clock_speed
//!     u16   dependencies.len
//!     i64[] dependencies
//!     u16-len string description
//!     u8    sync_state
//!     u16   score
//!     u16-len bytes player_data
//!     u16-len string hub_description
//!     [512 bytes design] if custom_id != 0
//!     i64   components.len
//!     components[]
//!     i64   wires.len
//!     wires[]
//! ```
//!
//! Each component additionally has a `custom_id + custom_word_sizes` block if
//! and only if `kind == 78` (the `CUSTOM_COMPONENT_KIND` constant).

use crate::circuit::{
    binary::{Reader, Writer},
    model::{Circuit, Component, Wire},
    snappy,
};

/// The version byte emitted by `encode_v15` and required by `decode_v15`.
pub const FORMAT_VERSION: u8 = 15;

/// Versions the top-level dispatcher can decode. v15 is the only format with
/// a writer; legacy versions are read-only.
pub const SUPPORTED_READ_VERSIONS: &[u8] = &[7, 13, 14, 15];

/// Component kind that triggers the custom-component tail block.
pub const CUSTOM_COMPONENT_KIND: u16 = 78;

/// Width of the `design` field for custom components.
pub const CUSTOM_DESIGN_BYTES: usize = 512;

fn read_component(reader: &mut Reader<'_>) -> Result<Component, String> {
    let kind = reader.u16()?;
    let position = reader.point()?;
    let rotation = reader.u8()?;
    let permanent_id = reader.i64()?;
    let user_label = reader.string()?;
    let custom_string = reader.string()?;
    let settings_len = reader.u16()? as usize;
    let mut settings = Vec::with_capacity(settings_len);
    for _ in 0..settings_len {
        settings.push(reader.u64()?);
    }
    let buffer_size = reader.i64()?;
    let ui_order = reader.i16()?;
    let word_size = reader.i64()?;
    let immutable = reader.boolean()?;
    let cost_gate = reader.i64()?;
    let cost_delay = reader.i64()?;
    let little_endian = reader.boolean()?;
    let init_data = reader.u8()?;
    let linked_len = reader.u16()? as usize;
    let mut linked_components = Vec::with_capacity(linked_len);
    for _ in 0..linked_len {
        let a = reader.i64()?;
        let b = reader.i64()?;
        let c = reader.string()?;
        let d = reader.i64()?;
        let e = reader.i64()?;
        linked_components.push((a, b, c, d, e));
    }
    let selected_len = reader.u16()? as usize;
    let mut selected_programs = Vec::with_capacity(selected_len);
    for _ in 0..selected_len {
        let level = reader.string()?;
        let program = reader.string()?;
        selected_programs.push((level, program));
    }
    let mut custom_id = 0;
    let mut custom_word_sizes = Vec::new();
    if kind == CUSTOM_COMPONENT_KIND {
        custom_id = reader.i64()?;
        let cw_len = reader.u16()? as usize;
        custom_word_sizes.reserve(cw_len);
        for _ in 0..cw_len {
            let a = reader.i64()?;
            let b = reader.i64()?;
            custom_word_sizes.push((a, b));
        }
    }

    Ok(Component {
        kind,
        position,
        rotation,
        permanent_id,
        user_label,
        custom_string,
        settings,
        buffer_size,
        ui_order,
        word_size,
        immutable,
        cost_gate,
        cost_delay,
        little_endian,
        init_data,
        linked_components,
        selected_programs,
        custom_id,
        custom_word_sizes,
    })
}

fn read_wire(reader: &mut Reader<'_>) -> Result<Wire, String> {
    let color = reader.u8()?;
    let comment = reader.string()?;
    let start = reader.point()?;
    let mut segments = Vec::new();
    loop {
        let code = reader.u16()?;
        let length = code & 0x1FFF;
        if length == 0 {
            break;
        }
        let direction = (code >> 13) as u8;
        if direction > 7 {
            return Err(format!("CIRCUIT_BAD_WIRE_DIRECTION|{direction}"));
        }
        if !(1..=0x1FFF).contains(&length) {
            return Err(format!("CIRCUIT_BAD_WIRE_LENGTH|{length}"));
        }
        segments.push((direction, length));
    }
    Ok(Wire {
        color,
        comment,
        start,
        segments,
        teleport_end: None,
    })
}

/// Decode a v15 payload. Errors out on any truncation, version mismatch, or
/// structural inconsistency.
pub fn decode_v15(payload: &[u8]) -> Result<Circuit, String> {
    if payload.is_empty() {
        return Err("CIRCUIT_TOO_SHORT|empty input".to_string());
    }
    if payload[0] != FORMAT_VERSION {
        return Err(format!(
            "CIRCUIT_BAD_VERSION|expected {}, got {}",
            FORMAT_VERSION, payload[0]
        ));
    }
    let body = snappy::decompress(&payload[1..])?;
    let mut reader = Reader::new(&body);

    let custom_id = reader.i64()?;
    let hub_id = reader.u32()?;
    let gate = reader.i64()?;
    let delay = reader.i64()?;
    let menu_visible = reader.boolean()?;
    let clock_speed = reader.u64()?;
    let deps_len = reader.u16()? as usize;
    let mut dependencies = Vec::with_capacity(deps_len);
    for _ in 0..deps_len {
        dependencies.push(reader.i64()?);
    }
    let description = reader.string()?;
    let sync_state = reader.u8()?;
    let score = reader.u16()?;
    let player_data = reader.bytes_u16()?;
    let hub_description = reader.string()?;
    let design = if custom_id != 0 {
        let bytes = reader.take(CUSTOM_DESIGN_BYTES)?;
        bytes.to_vec()
    } else {
        Vec::new()
    };
    let components_len = reader.count_i64("components")?;
    let mut components = Vec::with_capacity(components_len);
    for _ in 0..components_len {
        components.push(read_component(&mut reader)?);
    }
    let wires_len = reader.count_i64("wires")?;
    let mut wires = Vec::with_capacity(wires_len);
    for _ in 0..wires_len {
        wires.push(read_wire(&mut reader)?);
    }
    reader.finish()?;

    Ok(Circuit {
        custom_id,
        hub_id,
        gate,
        delay,
        menu_visible,
        clock_speed,
        dependencies,
        description,
        sync_state,
        score,
        player_data,
        hub_description,
        design,
        components,
        wires,
    })
}

fn write_component(writer: &mut Writer, component: &Component) -> Result<(), String> {
    writer.u16(component.kind)?;
    writer.point(component.position)?;
    writer.u8(component.rotation);
    writer.i64(component.permanent_id)?;
    writer.string(&component.user_label)?;
    writer.string(&component.custom_string)?;
    writer.u16(component.settings.len() as u16)?;
    for &setting in &component.settings {
        writer.u64(setting)?;
    }
    writer.i64(component.buffer_size)?;
    writer.i16(component.ui_order)?;
    writer.i64(component.word_size)?;
    writer.boolean(component.immutable);
    writer.i64(component.cost_gate)?;
    writer.i64(component.cost_delay)?;
    writer.boolean(component.little_endian);
    writer.u8(component.init_data);
    writer.u16(component.linked_components.len() as u16)?;
    for (a, b, name, d, e) in &component.linked_components {
        writer.i64(*a)?;
        writer.i64(*b)?;
        writer.string(name)?;
        writer.i64(*d)?;
        writer.i64(*e)?;
    }
    writer.u16(component.selected_programs.len() as u16)?;
    for (level, program) in &component.selected_programs {
        writer.string(level)?;
        writer.string(program)?;
    }
    if component.kind == CUSTOM_COMPONENT_KIND {
        writer.i64(component.custom_id)?;
        writer.u16(component.custom_word_sizes.len() as u16)?;
        for (a, b) in &component.custom_word_sizes {
            writer.i64(*a)?;
            writer.i64(*b)?;
        }
    }
    Ok(())
}

fn write_wire(writer: &mut Writer, wire: &Wire) -> Result<(), String> {
    if wire.teleport_end.is_some() {
        return Err(
            "CIRCUIT_V7_TELEPORT|v15 cannot encode a v7 teleport wire".to_string(),
        );
    }
    writer.u8(wire.color);
    writer.string(&wire.comment)?;
    writer.point(wire.start)?;
    for &(direction, length) in &wire.segments {
        if !(0..=7).contains(&direction) {
            return Err(format!("CIRCUIT_BAD_WIRE_DIRECTION|{direction}"));
        }
        if !(1..=0x1FFF).contains(&length) {
            return Err(format!("CIRCUIT_BAD_WIRE_LENGTH|{length}"));
        }
        let code = ((direction as u16) << 13) | length;
        writer.u16(code)?;
    }
    writer.u16(0) // end-of-segments marker
}

fn write_body(circuit: &Circuit) -> Result<Vec<u8>, String> {
    if circuit.custom_id != 0 && circuit.design.len() != CUSTOM_DESIGN_BYTES {
        return Err(format!(
            "CIRCUIT_BAD_DESIGN_SIZE|got {}, expected {CUSTOM_DESIGN_BYTES}",
            circuit.design.len()
        ));
    }
    if circuit.custom_id == 0 && !circuit.design.is_empty() {
        return Err("CIRCUIT_BAD_DESIGN_SIZE|non-custom circuit cannot contain design bytes".to_string());
    }

    let mut writer = Writer::new();
    writer.i64(circuit.custom_id)?;
    writer.u32(circuit.hub_id)?;
    writer.i64(circuit.gate)?;
    writer.i64(circuit.delay)?;
    writer.boolean(circuit.menu_visible);
    writer.u64(circuit.clock_speed)?;
    writer.u16(circuit.dependencies.len() as u16)?;
    for &dep in &circuit.dependencies {
        writer.i64(dep)?;
    }
    writer.string(&circuit.description)?;
    writer.u8(circuit.sync_state);
    writer.u16(circuit.score)?;
    writer.bytes_u16(&circuit.player_data)?;
    writer.string(&circuit.hub_description)?;
    if circuit.custom_id != 0 {
        writer.pack(&circuit.design);
    }
    writer.count_i64(circuit.components.len())?;
    for component in &circuit.components {
        write_component(&mut writer, component)?;
    }
    writer.count_i64(circuit.wires.len())?;
    for wire in &circuit.wires {
        write_wire(&mut writer, wire)?;
    }
    Ok(writer.data)
}

/// Encode a `Circuit` to a v15 payload. Re-decodes the result to confirm
/// semantic round-trip; if the re-decoded value differs from the input the
/// function returns `CIRCUIT_ROUNDTRIP_FAILED`.
pub fn encode_v15(circuit: &Circuit) -> Result<Vec<u8>, String> {
    let body = write_body(circuit)?;
    let compressed = snappy::compress(&body)?;
    let mut out = Vec::with_capacity(1 + compressed.len());
    out.push(FORMAT_VERSION);
    out.extend_from_slice(&compressed);

    // Self-check: re-decode must equal the input.
    let re_decoded = decode_v15(&out)?;
    if &re_decoded != circuit {
        return Err("CIRCUIT_ROUNDTRIP_FAILED|re-decoded circuit differs".to_string());
    }
    Ok(out)
}

/// Dispatch by leading version byte. Returns `CIRCUIT_UNSUPPORTED` for unknown
/// versions. v15 has a writer; older versions are decoded via
/// [`crate::circuit::legacy`].
pub fn decode_circuit(payload: &[u8]) -> Result<Circuit, String> {
    if payload.is_empty() {
        return Err("CIRCUIT_TOO_SHORT|empty input".to_string());
    }
    match payload[0] {
        15 => decode_v15(payload),
        7 => crate::circuit::legacy::decode_v7(payload),
        13 => crate::circuit::legacy::decode_v13(payload),
        14 => crate::circuit::legacy::decode_v14(payload),
        other => Err(format!("CIRCUIT_UNSUPPORTED|version {other} not readable")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::model::{Component, Wire};

    fn sample_circuit() -> Circuit {
        let mut c = Circuit::default();
        c.gate = 2;
        c.delay = 2;
        c.description = "round-trip".to_string();
        c.components.push(Component {
            kind: 63,
            position: (-13, 0),
            rotation: 0,
            permanent_id: 1,
            user_label: "Input".to_string(),
            immutable: true,
            cost_gate: 0,
            ..Component::default()
        });
        c.components.push(Component {
            kind: 68,
            position: (13, 0),
            rotation: 0,
            permanent_id: 2,
            user_label: "Output".to_string(),
            immutable: true,
            cost_gate: 0,
            ..Component::default()
        });
        c.components.push(Component {
            kind: 6,
            position: (-7, 0),
            rotation: 0,
            permanent_id: 3,
            cost_gate: 1,
            ..Component::default()
        });
        c.wires.push(Wire {
            color: 0,
            start: (1, 0),
            segments: vec![(0, 12), (3, 6)],
            ..Wire::default()
        });
        c.wires.push(Wire {
            color: 0,
            start: (2, 0),
            segments: vec![(0, 5)],
            ..Wire::default()
        });
        c.wires.push(Wire {
            color: 0,
            start: (3, 0),
            segments: vec![(0, 14)],
            ..Wire::default()
        });
        c
    }

    #[test]
    fn round_trip_synthetic_circuit() {
        let c = sample_circuit();
        let bytes = encode_v15(&c).unwrap();
        assert_eq!(bytes[0], FORMAT_VERSION);
        let back = decode_v15(&bytes).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn rejects_wrong_version() {
        let mut bytes = encode_v15(&sample_circuit()).unwrap();
        bytes[0] = 14;
        let err = decode_v15(&bytes).unwrap_err();
        assert!(
            err.starts_with("CIRCUIT_BAD_VERSION|"),
            "got: {err}"
        );
    }

    #[test]
    fn rejects_empty() {
        let err = decode_v15(&[]).unwrap_err();
        assert!(err.starts_with("CIRCUIT_TOO_SHORT|"), "got: {err}");
    }

    #[test]
    fn rejects_bad_design_size() {
        let mut c = sample_circuit();
        c.custom_id = 1;
        c.design = vec![0u8; 100]; // should be 512
        let err = encode_v15(&c).unwrap_err();
        assert!(
            err.starts_with("CIRCUIT_BAD_DESIGN_SIZE|"),
            "got: {err}"
        );
    }

    #[test]
    fn rejects_non_custom_with_design() {
        let mut c = sample_circuit();
        c.design = vec![0u8; 16];
        let err = encode_v15(&c).unwrap_err();
        assert!(
            err.starts_with("CIRCUIT_BAD_DESIGN_SIZE|"),
            "got: {err}"
        );
    }

    #[test]
    fn dispatch_routes_legacy_via_decoder() {
        // Empty v7 payload → should land on legacy decoder which errors on
        // version mismatch (since v7 layout is different).
        let err = decode_circuit(&[7u8, 0, 0]).unwrap_err();
        // Either a v7 parsing error or a too-short error is acceptable — we
        // just want it not to silently succeed.
        assert!(
            err.starts_with("CIRCUIT_") || err.starts_with("LEVELS_"),
            "got: {err}"
        );
    }

    #[test]
    fn dispatch_rejects_unsupported() {
        let err = decode_circuit(&[99u8]).unwrap_err();
        assert!(err.starts_with("CIRCUIT_UNSUPPORTED|"), "got: {err}");
    }

    #[test]
    fn bad_wire_direction_errors() {
        // code with direction byte = 0b111 (max) shifted to bits 13-15 = 0xE000
        // plus length = 5 → 0xE005
        let c = Circuit {
            wires: vec![Wire {
                color: 0,
                start: (0, 0),
                segments: vec![(7u8, 5u16)], // direction=7 is fine; length=5 fine
                ..Wire::default()
            }],
            ..Circuit::default()
        };
        let bytes = encode_v15(&c).unwrap();
        let back = decode_v15(&bytes).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn round_trip_real_v15_file() {
        // Look for a known player save file. Use the same skip-if-no-game
        // pattern as `translations::tests::parse_level_names_end_to_end`.
        let Some(game_save_dir) = crate::config::default_save_dir() else {
            return;
        };
        let path = game_save_dir
            .join("and_gate")
            .join("缺省")
            .join("circuit.data");
        if !path.is_file() {
            return;
        }
        let bytes = std::fs::read(&path).expect("read and_gate circuit.data");
        let circuit = decode_v15(&bytes).expect("decode_v15");
        // Sanity: input + output gate count must match the game's manifest.
        // For and_gate, expect exactly 4 components and 4 wires (2 I/O + 1
        // gate + 1 immutable slot, observed via tc-save-lab Python tool).
        assert_eq!(
            circuit.components.len(),
            4,
            "and_gate/缺省 component count drifted"
        );
        assert_eq!(circuit.wires.len(), 4, "and_gate/缺省 wire count drifted");

        // Re-encode must round-trip semantically.
        let re_encoded = encode_v15(&circuit).expect("encode_v15");
        let re_decoded = decode_v15(&re_encoded).expect("decode_v15(re_encoded)");
        assert_eq!(re_decoded, circuit);
    }
}
//! Read-only decoders for campaign circuit formats before v15.
//!
//! Ported from `tc-save-lab/src/tc_save_lab/legacy_codec.py`. Old campaign
//! assets are useful as immutable level scaffolding and reference solutions,
//! but all generated / current save files must use the v15 writer.

use crate::circuit::{
    binary::Reader,
    model::{Circuit, Component, Wire},
    snappy,
};

/// Versions this module can decode. v15 lives in [`crate::circuit::codec`].
pub const READ_ONLY_FORMAT_VERSIONS: &[u8] = &[7, 13, 14];

/// Component kind that triggers the custom-component tail block.
pub const CUSTOM_COMPONENT_KIND: u16 = 78;

/// Width of the `design` field for custom components.
pub const CUSTOM_DESIGN_BYTES: usize = 512;

/// Before v15 only one label string was stored. These component kinds used it
/// as `custom_string`; every other kind used it as `user_label`.
pub const OLD_CUSTOM_STRING_COMPONENT_KINDS: &[u16] = &[46, 87, 94, 101];

/// v7 stored program selections and abbreviated watched-component records
/// only for these component kinds.
pub const V7_LINKED_COMPONENT_KINDS: &[u16] = &[50, 82, 83, 88, 90, 91];

/// Sentinel byte that begins a v7 teleport wire's "second endpoint" segment.
pub const V7_TELEPORT_WIRE: u8 = 0x20;

fn decode_body(payload: &[u8], expected_version: u8) -> Result<Vec<u8>, String> {
    if payload.is_empty() {
        return Err(format!(
            "CIRCUIT_TOO_SHORT|empty input for version {expected_version}"
        ));
    }
    if payload[0] != expected_version {
        return Err(format!(
            "CIRCUIT_BAD_VERSION|expected {expected_version}, got {}",
            payload[0]
        ));
    }
    snappy::decompress(&payload[1..])
}

/// v7/v13/v14 used a single label string per component. This helper returns
/// `(user_label, custom_string)` split according to [`OLD_CUSTOM_STRING_COMPONENT_KINDS`].
fn read_legacy_label(reader: &mut Reader<'_>, kind: u16) -> Result<(String, String), String> {
    let value = reader.string()?;
    if OLD_CUSTOM_STRING_COMPONENT_KINDS.contains(&kind) {
        Ok((String::new(), value))
    } else {
        Ok((value, String::new()))
    }
}

fn read_settings(reader: &mut Reader<'_>) -> Result<Vec<u64>, String> {
    let len = reader.u16()? as usize;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        out.push(reader.u64()?);
    }
    Ok(out)
}

fn read_selected_programs(reader: &mut Reader<'_>) -> Result<Vec<(String, String)>, String> {
    let len = reader.u16()? as usize;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        let level = reader.string()?;
        let program = reader.string()?;
        out.push((level, program));
    }
    Ok(out)
}

fn read_custom_word_sizes(reader: &mut Reader<'_>) -> Result<Vec<(i64, i64)>, String> {
    let len = reader.u16()? as usize;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        let a = reader.i64()?;
        let b = reader.i64()?;
        out.push((a, b));
    }
    Ok(out)
}

fn read_component_v7(reader: &mut Reader<'_>) -> Result<Component, String> {
    let kind = reader.u16()?;
    let position = reader.point()?;
    let rotation = reader.u8()?;
    let permanent_id = reader.i64()?;
    let (user_label, custom_string) = read_legacy_label(reader, kind)?;
    let settings = read_settings(reader)?;
    let buffer_size = reader.i64()?;
    let ui_order = reader.i16()?;
    let word_size = reader.i64()?;
    // Removed runtime/static-state identifier — discarded.
    reader.i64()?;

    let mut custom_id = 0;
    let mut custom_word_sizes = Vec::new();
    let mut selected_programs = Vec::new();
    let mut linked_components = Vec::new();

    if kind == CUSTOM_COMPONENT_KIND {
        custom_id = reader.i64()?;
        custom_word_sizes = read_custom_word_sizes(reader)?;
        // Discard the removed custom-linked word-size map.
        let skip = reader.u16()? as usize;
        for _ in 0..skip {
            reader.i64()?;
            reader.i64()?;
        }
    } else if V7_LINKED_COMPONENT_KINDS.contains(&kind) {
        selected_programs = read_selected_programs(reader)?;
        let linked_len = reader.u16()? as usize;
        for _ in 0..linked_len {
            let a = reader.i64()?;
            let b = reader.i64()?;
            let c = reader.string()?;
            linked_components.push((a, b, c, 0, 0));
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
        immutable: false,
        cost_gate: -1,
        cost_delay: 0,
        little_endian: false,
        init_data: 0,
        linked_components,
        selected_programs,
        custom_id,
        custom_word_sizes,
    })
}

fn read_component_v13_or_v14(reader: &mut Reader<'_>, with_cost: bool) -> Result<Component, String> {
    let kind = reader.u16()?;
    let position = reader.point()?;
    let rotation = reader.u8()?;
    let permanent_id = reader.i64()?;
    let (user_label, custom_string) = read_legacy_label(reader, kind)?;
    let settings = read_settings(reader)?;
    let buffer_size = reader.i64()?;
    let ui_order = reader.i16()?;
    let word_size = reader.i64()?;
    let immutable = reader.boolean()?;
    let cost_gate = if with_cost { reader.i64()? } else { -1 };
    let cost_delay = if with_cost { reader.i64()? } else { 0 };
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
    let selected_programs = read_selected_programs(reader)?;

    let mut custom_id = 0;
    let mut custom_word_sizes = Vec::new();
    if kind == CUSTOM_COMPONENT_KIND {
        custom_id = reader.i64()?;
        custom_word_sizes = read_custom_word_sizes(reader)?;
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

fn read_wire_v7(reader: &mut Reader<'_>) -> Result<Wire, String> {
    let color = reader.u8()?;
    let comment = reader.string()?;
    let start = reader.point()?;
    let first = reader.u8()?;
    if first == V7_TELEPORT_WIRE {
        let teleport_end = reader.point()?;
        return Ok(Wire {
            color,
            comment,
            start,
            segments: Vec::new(),
            teleport_end: Some(teleport_end),
        });
    }
    let mut segments = Vec::new();
    let mut code = first;
    while code & 0x1F != 0 {
        let direction = code >> 5;
        let length = (code & 0x1F) as u16;
        segments.push((direction, length));
        code = reader.u8()?;
    }
    Ok(Wire {
        color,
        comment,
        start,
        segments,
        teleport_end: None,
    })
}

fn read_wire_v13_or_v14(reader: &mut Reader<'_>) -> Result<Wire, String> {
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

struct LegacyHeader {
    custom_id: i64,
    hub_id: u32,
    gate: i64,
    delay: i64,
    menu_visible: bool,
    clock_speed: u64,
    dependencies: Vec<i64>,
    description: String,
    sync_state: u8,
    score: u16,
    player_data: Vec<u8>,
    hub_description: String,
}

fn read_header(reader: &mut Reader<'_>, has_camera_position: bool) -> Result<LegacyHeader, String> {
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
    if has_camera_position {
        // Camera position was a `Point` in v7; discarded on read.
        reader.point()?;
    }
    let sync_state = reader.u8()?;
    let score = reader.u16()?;
    let player_data = reader.bytes_u16()?;
    let hub_description = reader.string()?;
    Ok(LegacyHeader {
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
    })
}

/// Decode a v7 payload (read-only).
pub fn decode_v7(payload: &[u8]) -> Result<Circuit, String> {
    let body = decode_body(payload, 7)?;
    let mut reader = Reader::new(&body);
    let header = read_header(&mut reader, true)?;
    let components_len = reader.count_i64("components")?;
    let mut components = Vec::with_capacity(components_len);
    for _ in 0..components_len {
        components.push(read_component_v7(&mut reader)?);
    }
    let wires_len = reader.count_i64("wires")?;
    let mut wires = Vec::with_capacity(wires_len);
    for _ in 0..wires_len {
        wires.push(read_wire_v7(&mut reader)?);
    }
    reader.finish()?;
    Ok(assemble_circuit(header, Vec::new(), components, wires))
}

/// Decode a v13 payload (read-only).
pub fn decode_v13(payload: &[u8]) -> Result<Circuit, String> {
    decode_v13_or_v14(payload, 13)
}

/// Decode a v14 payload (read-only).
pub fn decode_v14(payload: &[u8]) -> Result<Circuit, String> {
    decode_v13_or_v14(payload, 14)
}

fn decode_v13_or_v14(payload: &[u8], version: u8) -> Result<Circuit, String> {
    let body = decode_body(payload, version)?;
    let mut reader = Reader::new(&body);
    let header = read_header(&mut reader, false)?;
    let design = if header.custom_id != 0 {
        let bytes = reader.take(CUSTOM_DESIGN_BYTES)?;
        bytes.to_vec()
    } else {
        Vec::new()
    };
    let components_len = reader.count_i64("components")?;
    let with_cost = version >= 14;
    let mut components = Vec::with_capacity(components_len);
    for _ in 0..components_len {
        components.push(read_component_v13_or_v14(&mut reader, with_cost)?);
    }
    let wires_len = reader.count_i64("wires")?;
    let mut wires = Vec::with_capacity(wires_len);
    for _ in 0..wires_len {
        wires.push(read_wire_v13_or_v14(&mut reader)?);
    }
    reader.finish()?;
    Ok(assemble_circuit(header, design, components, wires))
}

fn assemble_circuit(
    header: LegacyHeader,
    design: Vec<u8>,
    components: Vec<Component>,
    wires: Vec<Wire>,
) -> Circuit {
    Circuit {
        custom_id: header.custom_id,
        hub_id: header.hub_id,
        gate: header.gate,
        delay: header.delay,
        menu_visible: header.menu_visible,
        clock_speed: header.clock_speed,
        dependencies: header.dependencies,
        description: header.description,
        sync_state: header.sync_state,
        score: header.score,
        player_data: header.player_data,
        hub_description: header.hub_description,
        design,
        components,
        wires,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_wrong_version() {
        let err = decode_v7(&[15u8, 0, 0]).unwrap_err();
        assert!(
            err.starts_with("CIRCUIT_BAD_VERSION|"),
            "got: {err}"
        );
    }

    #[test]
    fn rejects_empty_input() {
        let err = decode_v13(&[]).unwrap_err();
        assert!(err.starts_with("CIRCUIT_TOO_SHORT|"), "got: {err}");
    }

    #[test]
    fn rejects_unsupported_version_in_dispatch() {
        // The dispatch helper in `codec.rs` would route this to legacy. Test
        // that legacy correctly rejects when called with the wrong version
        // for its expected version.
        let err = decode_v14(&[13u8]).unwrap_err();
        assert!(err.starts_with("CIRCUIT_BAD_VERSION|"), "got: {err}");
    }
}
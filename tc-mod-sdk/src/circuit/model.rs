//! Lossless in-memory model for current Turing Complete circuit files.
//!
//! Ported from `tc-save-lab/src/tc_save_lab/model.py`. Field names match the
//! Python definitions; serde derives are added so the same structs can be
//! passed across the Tauri command boundary.

use serde::{Deserialize, Serialize};

/// `(target_pin_id, inner_pin_id, name, offset, word_size)` — describes how a
/// polymorphic port binds to an inner sub-component pin.
pub type LinkedComponent = (i64, i64, String, i64, i64);

/// `(level_id, program_id)` — only populated for architecture levels.
pub type SelectedProgram = (String, String);

/// `(custom_id, word_size)` — per-port width for a custom component.
pub type CustomWordSize = (i64, i64);

/// `(x, y)` integer point.
pub type Point = (i16, i16);

/// A single circuit component (gate, pin, wire endpoint, etc.).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    pub kind: u16,
    pub position: Point,
    pub rotation: u8,
    pub permanent_id: i64,
    #[serde(default)]
    pub user_label: String,
    #[serde(default)]
    pub custom_string: String,
    #[serde(default)]
    pub settings: Vec<u64>,
    #[serde(default)]
    pub buffer_size: i64,
    #[serde(default)]
    pub ui_order: i16,
    #[serde(default = "default_word_size")]
    pub word_size: i64,
    #[serde(default)]
    pub immutable: bool,
    #[serde(default = "default_cost_gate")]
    pub cost_gate: i64,
    #[serde(default)]
    pub cost_delay: i64,
    #[serde(default)]
    pub little_endian: bool,
    #[serde(default)]
    pub init_data: u8,
    #[serde(default)]
    pub linked_components: Vec<LinkedComponent>,
    #[serde(default)]
    pub selected_programs: Vec<SelectedProgram>,
    #[serde(default)]
    pub custom_id: i64,
    #[serde(default)]
    pub custom_word_sizes: Vec<CustomWordSize>,
}

fn default_word_size() -> i64 {
    1
}

fn default_cost_gate() -> i64 {
    -1
}

/// A wire: start point plus a chain of directional segments.
///
/// `teleport_end` is set only by the v7 legacy format (a deliberately
/// disconnected wire's second point). v15 always encodes `None` here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wire {
    pub color: u8,
    #[serde(default)]
    pub comment: String,
    pub start: Point,
    /// `(direction: u8 0..=7, length: u16 1..=0x1FFF)`
    pub segments: Vec<(u8, u16)>,
    #[serde(default)]
    pub teleport_end: Option<Point>,
}

/// Top-level container matching the v15 record layout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Circuit {
    #[serde(default)]
    pub custom_id: i64,
    #[serde(default)]
    pub hub_id: u32,
    #[serde(default)]
    pub gate: i64,
    #[serde(default)]
    pub delay: i64,
    #[serde(default = "default_menu_visible")]
    pub menu_visible: bool,
    #[serde(default = "default_clock_speed")]
    pub clock_speed: u64,
    #[serde(default)]
    pub dependencies: Vec<i64>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub sync_state: u8,
    #[serde(default)]
    pub score: u16,
    #[serde(default)]
    pub player_data: Vec<u8>,
    #[serde(default)]
    pub hub_description: String,
    #[serde(default)]
    pub design: Vec<u8>,
    #[serde(default)]
    pub components: Vec<Component>,
    #[serde(default)]
    pub wires: Vec<Wire>,
}

fn default_menu_visible() -> bool {
    true
}

fn default_clock_speed() -> u64 {
    10_000_000
}

impl Circuit {
    /// Total cost = gate count × delay.
    pub fn energy(&self) -> i64 {
        self.gate * self.delay
    }
}

impl Default for Component {
    fn default() -> Self {
        Self {
            kind: 0,
            position: (0, 0),
            rotation: 0,
            permanent_id: 0,
            user_label: String::new(),
            custom_string: String::new(),
            settings: Vec::new(),
            buffer_size: 0,
            ui_order: 0,
            word_size: 1,
            immutable: false,
            cost_gate: -1,
            cost_delay: 0,
            little_endian: false,
            init_data: 0,
            linked_components: Vec::new(),
            selected_programs: Vec::new(),
            custom_id: 0,
            custom_word_sizes: Vec::new(),
        }
    }
}

impl Default for Wire {
    fn default() -> Self {
        Self {
            color: 0,
            comment: String::new(),
            start: (0, 0),
            segments: Vec::new(),
            teleport_end: None,
        }
    }
}

impl Default for Circuit {
    fn default() -> Self {
        Self {
            custom_id: 0,
            hub_id: 0,
            gate: 0,
            delay: 0,
            menu_visible: true,
            clock_speed: 10_000_000,
            dependencies: Vec::new(),
            description: String::new(),
            sync_state: 0,
            score: 0,
            player_data: Vec::new(),
            hub_description: String::new(),
            design: Vec::new(),
            components: Vec::new(),
            wires: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_circuit_serializes() {
        let c = Circuit::default();
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("\"components\":[]"));
        assert!(json.contains("\"wires\":[]"));
        assert!(json.contains("\"menu_visible\":true"));
        assert!(json.contains("\"clock_speed\":10000000"));
    }

    #[test]
    fn energy_is_gate_times_delay() {
        let c = Circuit {
            gate: 3,
            delay: 4,
            ..Circuit::default()
        };
        assert_eq!(c.energy(), 12);
    }
}
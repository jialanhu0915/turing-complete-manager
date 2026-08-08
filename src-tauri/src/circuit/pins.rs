//! Auditable pin schemas and endpoint-based logical connectivity.
//!
//! Faithful port of `tc-save-lab/src/tc_save_lab/pins.py` (reference
//! implementation). Resolves a circuit's wiring into a directed component graph:
//! wires are unioned into logical networks by shared endpoints; each network's
//! output pin(s) drive its input pins; Kahn's algorithm yields a component
//! evaluation order (used by the DSL generator).
//!
//! Only component kinds whose geometry has been checked against current
//! circuits are listed here; unsupported kinds surface explicitly rather than
//! receiving guessed pins.

use std::collections::{HashMap, HashSet, VecDeque};

use super::model::{Circuit, Component, Point, Wire};

/// `(dx, dy)` for each wire-segment direction index 0..=7.
const DIRECTIONS: [(i16, i16); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];

/// Pin signal direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinDir {
    Input,
    Output,
    OutputTristate,
}

/// A pin's fixed offset relative to its component's position. `width: None`
/// means "use the component's `word_size`".
#[derive(Debug, Clone)]
pub struct PinSpec {
    pub name: String,
    pub direction: PinDir,
    pub offset: (i16, i16),
    pub width: Option<i64>,
}

/// A pin positioned in absolute circuit coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionedPin {
    pub component_index: usize,
    pub permanent_id: i64,
    pub component_kind: u16,
    pub name: String,
    pub direction: PinDir,
    pub width: i64,
    pub position: Point,
}

/// A logical wire network (a connected set of wire endpoints sharing pins).
#[derive(Debug, Clone)]
pub struct Net {
    pub id: usize,
    pub pins: Vec<PositionedPin>,
    /// The output/tristate pin driving this net, if any.
    pub driver: Option<PositionedPin>,
}

/// Result of connectivity resolution.
#[derive(Debug, Clone)]
pub struct Connectivity {
    pub unsupported_kinds: Vec<u16>,
    pub networks: Vec<Net>,
    pub unconnected_pins: Vec<PositionedPin>,
    pub connected_pin_count: usize,
    /// Directed edges `(driver_component_index, receiver_component_index)`.
    pub edges: Vec<(usize, usize)>,
    /// Component indices in evaluation order (topological; input components first).
    pub topo_order: Vec<usize>,
    pub unit_logic_depth: usize,
}

use PinDir::{Input, Output, OutputTristate};

fn pin(name: String, dir: PinDir, x: i16, y: i16, w: Option<i64>) -> PinSpec {
    PinSpec {
        name,
        direction: dir,
        offset: (x, y),
        width: w,
    }
}
fn inp(name: &str, x: i16, y: i16) -> PinSpec {
    pin(name.to_string(), Input, x, y, None)
}
fn inp1(name: &str, x: i16, y: i16) -> PinSpec {
    pin(name.to_string(), Input, x, y, Some(1))
}
fn outp(name: &str, x: i16, y: i16) -> PinSpec {
    pin(name.to_string(), Output, x, y, None)
}
fn outp1(name: &str, x: i16, y: i16) -> PinSpec {
    pin(name.to_string(), Output, x, y, Some(1))
}
fn trist(name: &str, x: i16, y: i16) -> PinSpec {
    pin(name.to_string(), OutputTristate, x, y, None)
}
fn trist1(name: &str, x: i16, y: i16) -> PinSpec {
    pin(name.to_string(), OutputTristate, x, y, Some(1))
}

/// N one-bit pins stacked vertically at `(offset_x, start_y + i)`.
fn stacked(prefix: &str, dir: PinDir, offset_x: i16, start_y: i16, n: i16) -> Vec<PinSpec> {
    (0..n)
        .map(|i| pin(format!("{prefix}{i}"), dir, offset_x, start_y + i, Some(1)))
        .collect()
}

/// Static per-kind pin schema table (port of `PIN_SCHEMAS`).
fn pin_schemas(kind: u16) -> Option<Vec<PinSpec>> {
    let specs: Vec<PinSpec> = match kind {
        1 => vec![outp1("out", 1, 0)],
        2 => vec![outp1("out", 1, 0)],
        3 => vec![inp1("in", -1, 0), outp1("out", 2, 0)],
        4 => vec![inp1("in0", -1, -1), inp1("in1", -1, 1), outp1("out", 2, 0)],
        5 => vec![inp1("in0", -1, -1), inp1("in1", -1, 0), inp1("in2", -1, 1), outp1("out", 2, 0)],
        6 => vec![inp1("in0", -1, -1), inp1("in1", -1, 1), outp1("out", 2, 0)],
        7 => vec![inp1("in0", -1, -1), inp1("in1", -1, 1), outp1("out", 2, 0)],
        8 => vec![inp1("in0", -1, -1), inp1("in1", -1, 0), inp1("in2", -1, 1), outp1("out", 2, 0)],
        9 => vec![inp1("in0", -1, -1), inp1("in1", -1, 1), outp1("out", 2, 0)],
        10 => vec![inp1("in0", -1, -1), inp1("in1", -1, 1), outp1("out", 2, 0)],
        11 => vec![inp1("in0", -1, -1), inp1("in1", -1, 1), outp1("out", 2, 0)],
        12 => vec![inp1("enable", 0, 1), inp1("in", -1, 0), trist1("out", 2, 0)],
        13 => vec![inp1("in", -3, 0), outp1("out", 3, 0)],
        14 => vec![inp1("save", -3, -3), inp1("in", -3, 0), outp1("out", 3, 0)],
        15 => vec![
            inp1("carry_in", -1, -1),
            inp1("in0", -1, 0),
            inp1("in1", -1, 1),
            outp1("sum", 1, 0),
            outp1("carry_out", 1, 1),
        ],
        16 => {
            let mut v = stacked("in", Input, -1, -3, 8);
            v.push(pin("out".to_string(), Output, 1, 0, Some(8)));
            v
        }
        17 => {
            let mut v = vec![pin("in".to_string(), Input, -1, 0, Some(8))];
            v.extend(stacked("out", Output, 1, -3, 8));
            v
        }
        18 => vec![inp("in", -1, 0), outp("out", 2, 0)],
        19 | 20 | 21 | 22 | 23 | 24 => {
            vec![inp("in0", -1, -1), inp("in1", -1, 1), outp("out", 2, 0)]
        }
        25 => vec![inp1("enable", 0, 1), inp("in", -1, 0), trist("out", 2, 0)],
        26 | 27 | 28 => vec![inp("in0", -1, -1), inp("in1", -1, 1), outp1("out", 2, 0)],
        29 => vec![inp("in", -1, 0), outp("out", 2, 0)],
        30 => vec![
            inp("in0", -1, -1),
            inp("in1", -1, 1),
            inp1("carry_in", 0, -2),
            outp("out", 2, 0),
            outp("carry_out", 1, 2),
        ],
        31 => vec![
            inp("in0", -1, -1),
            inp("in1", -1, 0),
            outp("low", 1, -1),
            outp("high", 1, 0),
        ],
        32 => vec![inp("in0", -1, -1), inp("in1", -1, 1), outp("out", 2, 0)],
        33 | 34 | 35 | 36 | 37 => vec![
            inp("in", -1, -1),
            pin("shift".to_string(), Input, -1, 1, Some(8)),
            outp("out", 2, 0),
        ],
        39 => vec![inp1("save", -3, -1), inp("in", -3, 0), outp("out", 3, 0)],
        40 => stacked("value", Input, -1, -4, 8),
        42 => vec![
            inp1("select", -1, -1),
            inp("in0", -1, 0),
            inp("in1", -1, 1),
            outp("out", 2, 0),
        ],
        43 => vec![inp1("select", -1, 0), outp1("out0", 1, 0), outp1("out1", 1, 1)],
        44 => {
            let mut v = vec![inp1("select0", -1, -1), inp1("select1", -1, 0)];
            v.extend(stacked("out", Output, 1, -1, 4));
            v
        }
        45 => {
            let mut v = vec![inp1("disable", 0, -4)];
            v.extend(stacked("select", Input, -1, -3, 3));
            v.extend(stacked("out", Output, 1, -3, 8));
            v
        }
        49 => vec![inp("in", -1, 0), outp("out", 2, 0)],
        54 => vec![
            inp1("enable", -15, -1),
            pin("address".to_string(), Input, -15, 0, Some(32)),
            outp("out", 16, -1),
        ],
        56 => vec![
            inp1("enable", -15, -1),
            pin("address".to_string(), Input, -15, 0, Some(32)),
            inp("data", -15, 1),
        ],
        60 => vec![outp1("value", 1, 0)],
        63 => vec![outp1("value0", 0, -1), outp1("value1", 0, 1)],
        64 => vec![outp1("value0", 1, -2), outp1("value1", 1, -1), outp1("value2", 1, 0)],
        65 => vec![
            outp1("value0", 1, -2),
            outp1("value1", 1, -1),
            outp1("value2", 1, 0),
            outp1("value3", 1, 1),
        ],
        68 => vec![inp1("value", -1, 0)],
        73 => vec![inp1("value0", -1, -1), inp1("value1", -1, 0)],
        74 => vec![inp1("value0", -1, -1), inp1("value1", -1, 0), inp1("value2", -1, 1)],
        75 => vec![
            inp1("value0", -1, -2),
            inp1("value1", -1, -1),
            inp1("value2", -1, 0),
            inp1("value3", -1, 1),
        ],
        77 => vec![inp1("value0", -1, -1), inp1("value1", -1, 0), inp1("value2", -1, 1)],
        79 => vec![pin("in".to_string(), Output, 3, 0, None)],
        81 => vec![pin("out".to_string(), Input, -3, 0, None)],
        97 => {
            let mut v = vec![];
            for i in 0..4 {
                v.push(pin(format!("in{i}"), Input, -1, i - 1, Some(8)));
            }
            v.push(pin("out".to_string(), Output, 1, 0, Some(32)));
            v
        }
        98 => {
            let mut v = vec![];
            for i in 0..8 {
                v.push(pin(format!("in{i}"), Input, -1, i - 3, Some(8)));
            }
            v.push(pin("out".to_string(), Output, 1, 0, Some(64)));
            v
        }
        99 => {
            let mut v = vec![pin("in".to_string(), Input, -1, 0, Some(32))];
            for i in 0..4 {
                v.push(pin(format!("out{i}"), Output, 1, i - 1, Some(8)));
            }
            v
        }
        100 => {
            let mut v = vec![pin("in".to_string(), Input, -1, 0, Some(64))];
            for i in 0..8 {
                v.push(pin(format!("out{i}"), Output, 1, i - 3, Some(8)));
            }
            v
        }
        109 => vec![
            pin("in".to_string(), Input, -1, 0, Some(2)),
            outp1("out0", 1, -1),
            outp1("out1", 1, 0),
        ],
        111 => vec![
            inp1("in0", -1, -1),
            inp1("in1", -1, 0),
            pin("out".to_string(), Output, 1, 0, Some(2)),
        ],
        118 => vec![],
        _ => return None,
    };
    Some(specs)
}

/// Resolve a component's pin layout (per-kind special cases that depend on
/// `word_size` / geometry evidence beyond the static table).
fn pin_specs_for(component: &Component) -> Option<Vec<PinSpec>> {
    if component.kind == 46 {
        return Some(vec![pin("out".to_string(), Output, 3, 0, None)]);
    }
    if component.kind == 61 {
        return Some(vec![pin("value".to_string(), Output, 3, 0, None)]);
    }
    if component.kind == 62 {
        return Some(vec![
            pin("control".to_string(), Input, 1, -2, Some(1)),
            pin("value".to_string(), OutputTristate, 3, 0, None),
        ]);
    }
    if component.kind == 55 {
        if !(1..=64).contains(&component.word_size) {
            return None;
        }
        let span: i16 = if component.word_size <= 8 {
            1
        } else if component.word_size <= 32 {
            2
        } else {
            3
        };
        return Some(vec![
            pin("in".to_string(), Input, -span, 0, None),
            pin("out".to_string(), Output, span, 0, None),
        ]);
    }
    if (33..=37).contains(&component.kind) {
        // Shift components consume only the bits needed to encode a shift count.
        let bits = (component.word_size.max(2) - 1).ilog2() as i64 + 1;
        let shift_width = bits.max(1);
        return Some(vec![
            pin("in".to_string(), Input, -1, -1, None),
            pin("shift".to_string(), Input, -1, 1, Some(shift_width)),
            pin("out".to_string(), Output, 2, 0, None),
        ]);
    }
    if component.kind == 69 {
        return Some(vec![pin("value".to_string(), Input, -3, 0, None)]);
    }
    if component.kind == 70 {
        return Some(vec![
            pin("control".to_string(), Input, -1, -2, Some(1)),
            pin("value".to_string(), Input, -3, 0, None),
        ]);
    }
    pin_schemas(component.kind)
}

fn rotate_offset(offset: (i16, i16), rotation: u8) -> (i16, i16) {
    let (x, y) = offset;
    match rotation & 0x3 {
        0 => (x, y),
        1 => (-y, x),
        2 => (-x, -y),
        _ => (y, -x),
    }
}

fn positioned_pins(component: &Component, component_index: usize) -> Vec<PositionedPin> {
    let Some(specs) = pin_specs_for(component) else {
        return Vec::new();
    };
    specs
        .iter()
        .map(|spec| {
            let (dx, dy) = rotate_offset(spec.offset, component.rotation);
            PositionedPin {
                component_index,
                permanent_id: component.permanent_id,
                component_kind: component.kind,
                name: spec.name.clone(),
                direction: spec.direction,
                width: spec.width.unwrap_or(component.word_size),
                position: (component.position.0 + dx, component.position.1 + dy),
            }
        })
        .collect()
}

/// Expand a wire's path to grid points. The first and last points are its
/// logical endpoints (the positions pins sit at).
fn wire_points(wire: &Wire) -> Vec<Point> {
    let mut points = vec![wire.start];
    if let Some(end) = wire.teleport_end {
        points.push(end);
        return points;
    }
    let (mut x, mut y) = wire.start;
    for &(direction, length) in &wire.segments {
        let (dx, dy) = DIRECTIONS[direction as usize % 8];
        for _ in 0..length {
            x += dx;
            y += dy;
            points.push((x, y));
        }
    }
    points
}

struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    fn new(size: usize) -> Self {
        UnionFind {
            parent: (0..size).collect(),
        }
    }
    fn find(&mut self, value: usize) -> usize {
        let mut v = value;
        while self.parent[v] != v {
            self.parent[v] = self.parent[self.parent[v]];
            v = self.parent[v];
        }
        v
    }
    fn union(&mut self, left: usize, right: usize) {
        let l = self.find(left);
        let r = self.find(right);
        if l != r {
            self.parent[r] = l;
        }
    }
}

/// Kinds whose input pins are sequential (register save/load) — skipped when
/// forming directed edges because they'd create false combinational loops.
const SEQUENTIAL_KINDS: &[u16] = &[13, 14, 38, 39, 50, 55, 118, 119];
/// Kinds treated as zero-weight (level I/O) in depth computation.
const OUTPUT_KINDS: &[u16] = &[40, 62, 68, 69, 70, 73, 74, 75, 77, 79, 81, 109, 110, 111, 112];

/// Resolve the circuit's wiring into a directed component graph.
///
/// Returns an error listing unsupported component kinds so the caller can
/// report them instead of silently producing a wrong topology.
pub fn resolve(circuit: &Circuit) -> Result<Connectivity, String> {
    let components = &circuit.components;
    let mut unsupported = Vec::new();
    for (i, component) in components.iter().enumerate() {
        if pin_specs_for(component).is_none() {
            unsupported.push((i, component.kind));
        }
    }
    if !unsupported.is_empty() {
        let kinds: Vec<String> = unsupported.iter().map(|(_, k)| k.to_string()).collect();
        return Err(format!("UNSUPPORTED_COMPONENT_KINDS|{}", kinds.join(",")));
    }

    // Wire endpoints, unioned by shared position into logical networks.
    let mut owners: HashMap<Point, Vec<usize>> = HashMap::new();
    for (index, wire) in circuit.wires.iter().enumerate() {
        let points = wire_points(wire);
        let endpoints = (points[0], *points.last().unwrap());
        owners.entry(endpoints.0).or_default().push(index);
        owners.entry(endpoints.1).or_default().push(index);
    }
    let mut uf = UnionFind::new(circuit.wires.len());
    for wire_indices in owners.values() {
        for &wire_index in &wire_indices[1..] {
            uf.union(wire_indices[0], wire_index);
        }
    }

    let mut network_for_position: HashMap<Point, usize> = HashMap::new();
    for (index, wire) in circuit.wires.iter().enumerate() {
        let points = wire_points(wire);
        let root = uf.find(index);
        network_for_position.insert(points[0], root);
        network_for_position.insert(*points.last().unwrap(), root);
    }

    // Position every component's pins and attach them to networks by position.
    let mut network_pins: HashMap<usize, Vec<PositionedPin>> = HashMap::new();
    let mut unconnected: Vec<PositionedPin> = Vec::new();
    let mut connected = 0usize;
    for (index, component) in components.iter().enumerate() {
        for pin in positioned_pins(component, index) {
            if let Some(&network) = network_for_position.get(&pin.position) {
                network_pins.entry(network).or_default().push(pin);
                connected += 1;
            } else {
                unconnected.push(pin);
            }
        }
    }

    let mut networks: Vec<Net> = Vec::new();
    for (id, mut pins) in network_pins.into_iter() {
        pins.sort_by_key(|p| (p.component_index, p.name.clone()));
        let driver = pins
            .iter()
            .find(|p| matches!(p.direction, PinDir::Output | PinDir::OutputTristate))
            .cloned();
        networks.push(Net { id, pins, driver });
    }

    // Directed edges: each network's driver component → its receiver components.
    let mut edges: HashSet<(usize, usize)> = HashSet::new();
    for net in &networks {
        let Some(driver) = &net.driver else { continue };
        for receiver in &net.pins {
            if receiver.direction != PinDir::Input {
                continue;
            }
            if receiver.component_index == driver.component_index {
                continue;
            }
            if SEQUENTIAL_KINDS.contains(&receiver.component_kind) {
                continue;
            }
            edges.insert((driver.component_index, receiver.component_index));
        }
    }

    // Kahn's topological sort with a 0/1 depth weight (level I/O are weight 0).
    let mut successors: HashMap<usize, HashSet<usize>> = HashMap::new();
    let mut indegree = vec![0usize; components.len()];
    for &(source, dest) in &edges {
        if successors.get(&source).map_or(true, |s| !s.contains(&dest)) {
            successors.entry(source).or_default().insert(dest);
            indegree[dest] += 1;
        }
    }
    let mut queue: VecDeque<usize> = (0..components.len())
        .filter(|&i| indegree[i] == 0)
        .collect();
    let mut topo: Vec<usize> = Vec::with_capacity(components.len());
    let mut depths = vec![0usize; components.len()];
    let mut visited = 0usize;
    while let Some(source) = queue.pop_front() {
        topo.push(source);
        visited += 1;
        let mut next: Vec<usize> = successors
            .get(&source)
            .into_iter()
            .flatten()
            .copied()
            .collect();
        next.sort_unstable();
        for dest in next {
            let weight = if OUTPUT_KINDS.contains(&components[dest].kind) {
                0
            } else {
                1
            };
            depths[dest] = depths[dest].max(depths[source] + weight);
            indegree[dest] -= 1;
            if indegree[dest] == 0 {
                queue.push_back(dest);
            }
        }
    }

    let unsupported_kinds: Vec<u16> = unsupported.iter().map(|(_, k)| *k).collect();
    Ok(Connectivity {
        unsupported_kinds,
        networks,
        unconnected_pins: unconnected,
        connected_pin_count: connected,
        edges: edges.into_iter().collect(),
        topo_order: topo,
        unit_logic_depth: depths.iter().copied().max().unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_poc(level: &str) -> Circuit {
        let path = format!(
            "{}/Turing Complete/schematics/{level}/缺省/circuit.data",
            std::env::var("APPDATA").unwrap()
        );
        let payload = std::fs::read(&path).expect("read circuit.data");
        super::super::codec::decode_circuit(&payload).expect("decode circuit")
    }

    #[test]
    fn wire_direction_4_is_west() {
        // start (-3,0), dir 4 (west), len 2 → end (-5,0).
        let wire = Wire {
            start: (-3, 0),
            segments: vec![(4, 2)],
            ..Wire::default()
        };
        let pts = wire_points(&wire);
        assert_eq!(*pts.last().unwrap(), (-5, 0));
        assert_eq!(pts.len(), 3);
    }

    #[test]
    #[ignore = "needs the player's save directory; run via --ignored"]
    fn and_gate_chain_matches_game() {
        let c = load_poc("and_gate");
        let conn = resolve(&c).expect("resolve and_gate");
        // Input(0) → nand(2) → not(3) → Output(1), per the Phase 1 spike.
        assert!(conn.edges.contains(&(0, 2)), "input drives nand: {:?}", conn.edges);
        assert!(conn.edges.contains(&(2, 3)), "nand drives not: {:?}", conn.edges);
        assert!(conn.edges.contains(&(3, 1)), "not drives output: {:?}", conn.edges);
        assert_eq!(conn.edges.len(), 3, "exactly 3 combinational edges");
    }

    #[test]
    #[ignore = "needs the player's save directory; run via --ignored"]
    fn or_gate_chain_is_de_morgan() {
        let c = load_poc("or_gate");
        let conn = resolve(&c).expect("resolve or_gate");
        // A,B → not(2,3) → nand(4) → Output(1): edges 0→2,0→3,2→4,3→4,4→1.
        assert_eq!(conn.edges.len(), 5, "edges: {:?}", conn.edges);
        assert!(conn.edges.contains(&(4, 1)));
    }

    #[test]
    #[ignore = "needs the player's save directory; run via --ignored"]
    fn not_gate_chain_is_self_tied_nand() {
        let c = load_poc("not_gate");
        let conn = resolve(&c).expect("resolve not_gate");
        // Input(0) → nand(2, self-tied) → Output(1).
        assert!(conn.edges.contains(&(0, 2)), "edges: {:?}", conn.edges);
        assert!(conn.edges.contains(&(2, 1)), "edges: {:?}", conn.edges);
    }
}

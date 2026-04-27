//! Typed representation of a Spectra DSN PCB file.

use pest::{iterators::Pair, Parser};
use pest_derive::Parser;

// ─── Grammar ─────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[grammar = "dsn.pest"]
struct DsnParser;

// ─── S-expression tree ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum SExpr {
    Atom(String),
    List(Vec<SExpr>),
}

impl SExpr {
    pub fn tag(&self) -> &str {
        match self {
            SExpr::List(items) => match items.first() {
                Some(SExpr::Atom(s)) => s.as_str(),
                _ => "",
            },
            _ => "",
        }
    }

    pub fn children(&self) -> &[SExpr] {
        match self {
            SExpr::List(items) => &items[1..],
            _ => &[],
        }
    }

    pub fn as_atom(&self) -> Option<&str> {
        match self {
            SExpr::Atom(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// All immediate Atom children (skips nested lists).
    pub fn atom_children(&self) -> Vec<&str> {
        match self {
            SExpr::List(items) => items[1..]
                .iter()
                .filter_map(|e| e.as_atom())
                .collect(),
            _ => vec![],
        }
    }

    /// All immediate List children with the given tag.
    pub fn find_all<'a>(&'a self, tag: &str) -> Vec<&'a SExpr> {
        match self {
            SExpr::List(items) => items[1..]
                .iter()
                .filter(|e| e.tag() == tag)
                .collect(),
            _ => vec![],
        }
    }

    pub fn find_first(&self, tag: &str) -> Option<&SExpr> {
        self.find_all(tag).into_iter().next()
    }
}

fn convert_pair(pair: Pair<Rule>) -> Option<SExpr> {
    match pair.as_rule() {
        Rule::iden => Some(SExpr::Atom(pair.as_str().to_string())),
        Rule::string => {
            let raw = pair.as_str();
            // Strip surrounding quotes
            let inner = &raw[1..raw.len().saturating_sub(1)];
            Some(SExpr::Atom(inner.to_string()))
        }
        Rule::sexpr => {
            let children: Vec<SExpr> = pair.into_inner().filter_map(convert_pair).collect();
            Some(SExpr::List(children))
        }
        Rule::expr => pair.into_inner().next().and_then(convert_pair),
        _ => None,
    }
}

// ─── DSN types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct Layer {
    pub name: String,
    pub layer_type: String,
    pub index: Option<u32>,
}

#[derive(Debug, Clone)]
pub enum Shape {
    Path {
        layer: String,
        width: f64,
        coords: Vec<(f64, f64)>,
    },
    Polygon {
        layer: String,
        width: f64,
        coords: Vec<(f64, f64)>,
    },
    Rect {
        layer: String,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
    Circle {
        layer: String,
        diameter: f64,
        cx: f64,
        cy: f64,
    },
}

#[derive(Debug, Clone, Default)]
pub struct RoutingRule {
    pub width: f64,
    /// (clearance_value, optional type label)
    pub clearances: Vec<(f64, Option<String>)>,
}

#[derive(Debug, Clone, Default)]
pub struct Structure {
    pub layers: Vec<Layer>,
    pub boundary: Option<Shape>,
    pub vias: Vec<String>,
    pub rules: Vec<RoutingRule>,
    pub keepouts: Vec<Shape>,
    pub planes: Vec<(String, Shape)>,
}

#[derive(Debug, Clone, Default)]
pub struct Pin {
    pub padstack: String,
    pub name: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Image {
    pub name: String,
    pub pins: Vec<Pin>,
}

#[derive(Debug, Clone, Default)]
pub struct Padstack {
    pub name: String,
    pub shapes: Vec<Shape>,
    pub attach: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Library {
    pub images: Vec<Image>,
    pub padstacks: Vec<Padstack>,
}

#[derive(Debug, Clone, Default)]
pub struct PlacedComponent {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub side: String,
    pub rotation: f64,
}

#[derive(Debug, Clone, Default)]
pub struct ComponentGroup {
    pub image: String,
    pub places: Vec<PlacedComponent>,
}

#[derive(Debug, Clone, Default)]
pub struct Placement {
    pub components: Vec<ComponentGroup>,
}

#[derive(Debug, Clone, Default)]
pub struct Net {
    pub name: String,
    pub pins: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct NetClass {
    pub name: String,
    pub nets: Vec<String>,
    pub via: Option<String>,
    pub rule: Option<RoutingRule>,
}

#[derive(Debug, Clone, Default)]
pub struct Network {
    pub nets: Vec<Net>,
    pub classes: Vec<NetClass>,
}

#[derive(Debug, Clone, Default)]
pub struct Wire {
    pub layer: String,
    pub width: f64,
    pub path: Vec<(f64, f64)>,
    pub net: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PlacedVia {
    pub padstack: String,
    pub x: f64,
    pub y: f64,
    pub net: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Wiring {
    pub wires: Vec<Wire>,
    pub vias: Vec<PlacedVia>,
}

// ─── Top-level PCB ───────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone)]
pub struct Pcb {
    pub id: String,
    pub resolution_unit: String,
    pub resolution_value: f64,
    pub unit: String,
    pub structure: Structure,
    pub placement: Placement,
    pub library: Library,
    pub network: Network,
    pub wiring: Wiring,
}

// ─── Parsing helpers ─────────────────────────────────────────────────────────

fn parse_f64(s: &str) -> f64 {
    s.parse().unwrap_or(0.0)
}

fn parse_coords(atoms: &[&str]) -> Vec<(f64, f64)> {
    atoms
        .chunks(2)
        .filter_map(|c| {
            if c.len() == 2 {
                Some((parse_f64(c[0]), parse_f64(c[1])))
            } else {
                None
            }
        })
        .collect()
}

fn parse_shape(expr: &SExpr) -> Option<Shape> {
    let tag = expr.tag();
    let atoms: Vec<&str> = expr.atom_children();
    match tag {
        "path" | "polyline_path" | "qarc" => {
            // (path layer width x1 y1 x2 y2 ...)
            if atoms.len() < 2 {
                return None;
            }
            let layer = atoms[0].to_string();
            let width = parse_f64(atoms[1]);
            let coords = parse_coords(&atoms[2..]);
            Some(Shape::Path { layer, width, coords })
        }
        "polygon" => {
            if atoms.len() < 2 {
                return None;
            }
            let layer = atoms[0].to_string();
            let width = parse_f64(atoms[1]);
            let coords = parse_coords(&atoms[2..]);
            Some(Shape::Polygon { layer, width, coords })
        }
        "rect" => {
            if atoms.len() < 5 {
                return None;
            }
            Some(Shape::Rect {
                layer: atoms[0].to_string(),
                x1: parse_f64(atoms[1]),
                y1: parse_f64(atoms[2]),
                x2: parse_f64(atoms[3]),
                y2: parse_f64(atoms[4]),
            })
        }
        "circle" => {
            let layer = atoms.first().copied().unwrap_or("").to_string();
            let diameter = atoms.get(1).map(|s| parse_f64(s)).unwrap_or(0.0);
            let cx = atoms.get(2).map(|s| parse_f64(s)).unwrap_or(0.0);
            let cy = atoms.get(3).map(|s| parse_f64(s)).unwrap_or(0.0);
            Some(Shape::Circle { layer, diameter, cx, cy })
        }
        _ => None,
    }
}

fn parse_routing_rule(expr: &SExpr) -> RoutingRule {
    let mut rule = RoutingRule::default();
    if let SExpr::List(items) = expr {
        for item in &items[1..] {
            match item.tag() {
                "width" => {
                    if let Some(v) = item.atom_children().first() {
                        rule.width = parse_f64(v);
                    }
                }
                "clearance" => {
                    let atoms = item.atom_children();
                    let val = atoms.first().map(|s| parse_f64(s)).unwrap_or(0.0);
                    let type_label = item
                        .find_first("type")
                        .and_then(|t| t.atom_children().first().copied())
                        .map(String::from);
                    rule.clearances.push((val, type_label));
                }
                _ => {}
            }
        }
    }
    rule
}

fn parse_structure(expr: &SExpr) -> Structure {
    let mut s = Structure::default();
    if let SExpr::List(items) = expr {
        for item in &items[1..] {
            match item.tag() {
                "layer" => {
                    let atoms = item.atom_children();
                    let name = atoms.first().copied().unwrap_or("").to_string();
                    let layer_type = item
                        .find_first("type")
                        .and_then(|t| t.atom_children().first().copied())
                        .unwrap_or("signal")
                        .to_string();
                    let index = item
                        .find_first("property")
                        .and_then(|p| p.find_first("index"))
                        .and_then(|i| i.atom_children().first().copied())
                        .and_then(|v| v.parse().ok());
                    s.layers.push(Layer { name, layer_type, index });
                }
                "boundary" => {
                    if let Some(shape_expr) = item.children().iter().find(|c| {
                        matches!(c.tag(), "path" | "polygon" | "rect" | "circle")
                    }) {
                        s.boundary = parse_shape(shape_expr);
                    }
                }
                "via" => {
                    if let Some(name) = item.atom_children().first() {
                        s.vias.push(name.to_string());
                    }
                }
                "rule" => s.rules.push(parse_routing_rule(item)),
                "keepout" => {
                    if let Some(shape_expr) = item.children().iter().find(|c| {
                        matches!(c.tag(), "path" | "polygon" | "rect" | "circle")
                    }) {
                        if let Some(shape) = parse_shape(shape_expr) {
                            s.keepouts.push(shape);
                        }
                    }
                }
                "plane" => {
                    let atoms = item.atom_children();
                    let net = atoms.first().copied().unwrap_or("").to_string();
                    if let Some(shape_expr) = item.children().iter().skip(1).find(|c| {
                        matches!(c.tag(), "path" | "polygon" | "rect" | "circle")
                    }) {
                        if let Some(shape) = parse_shape(shape_expr) {
                            s.planes.push((net, shape));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    s
}

fn parse_placement(expr: &SExpr) -> Placement {
    let mut p = Placement::default();
    for comp in expr.find_all("component") {
        let image = comp.atom_children().first().copied().unwrap_or("").to_string();
        let mut places = vec![];
        for place in comp.find_all("place") {
            let atoms = place.atom_children();
            if atoms.len() >= 5 {
                places.push(PlacedComponent {
                    id: atoms[0].to_string(),
                    x: parse_f64(atoms[1]),
                    y: parse_f64(atoms[2]),
                    side: atoms[3].to_string(),
                    rotation: parse_f64(atoms[4]),
                });
            }
        }
        p.components.push(ComponentGroup { image, places });
    }
    p
}

fn parse_library(expr: &SExpr) -> Library {
    let mut lib = Library::default();

    for image_expr in expr.find_all("image") {
        let name = image_expr.atom_children().first().copied().unwrap_or("").to_string();
        let mut pins = vec![];
        for pin_expr in image_expr.find_all("pin") {
            let atoms = pin_expr.atom_children();
            if atoms.len() >= 4 {
                pins.push(Pin {
                    padstack: atoms[0].to_string(),
                    name: atoms[1].to_string(),
                    x: parse_f64(atoms[2]),
                    y: parse_f64(atoms[3]),
                });
            }
        }
        lib.images.push(Image { name, pins });
    }

    for ps_expr in expr.find_all("padstack") {
        let name = ps_expr.atom_children().first().copied().unwrap_or("").to_string();
        let attach = ps_expr
            .find_first("attach")
            .and_then(|a| a.atom_children().first().copied())
            .map(|v| v == "on")
            .unwrap_or(false);
        let mut shapes = vec![];
        for shape_expr in ps_expr.find_all("shape") {
            if let Some(inner) = shape_expr.children().first() {
                if let Some(shape) = parse_shape(inner) {
                    shapes.push(shape);
                }
            }
        }
        lib.padstacks.push(Padstack { name, shapes, attach });
    }

    lib
}

fn parse_network(expr: &SExpr) -> Network {
    let mut net = Network::default();

    for net_expr in expr.find_all("net") {
        let name = net_expr.atom_children().first().copied().unwrap_or("").to_string();
        let pins = net_expr
            .find_first("pins")
            .map(|p| p.atom_children().iter().map(|s| s.to_string()).collect())
            .unwrap_or_default();
        net.nets.push(Net { name, pins });
    }

    for class_expr in expr.find_all("class") {
        let atoms = class_expr.atom_children();
        let name = atoms.first().copied().unwrap_or("").to_string();
        // Remaining atoms (after name + optional desc) that don't match sub-lists are net names.
        // The class sexpr has format: (class "name" "desc" "net1" "net2" ... (circuit ...) (rule ...))
        // atom_children skips all sub-lists, so atoms[2..] are net names.
        let nets: Vec<String> = if atoms.len() > 2 {
            atoms[2..].iter().map(|s| s.to_string()).collect()
        } else {
            vec![]
        };
        let via = class_expr
            .find_first("circuit")
            .and_then(|c| c.find_first("use_via"))
            .and_then(|v| v.atom_children().first().copied())
            .map(String::from);
        let rule = class_expr.find_first("rule").map(parse_routing_rule);
        net.classes.push(NetClass { name, nets, via, rule });
    }

    net
}

fn parse_wiring(expr: &SExpr) -> Wiring {
    let mut wiring = Wiring::default();

    for wire_expr in expr.find_all("wire") {
        let path_expr = wire_expr
            .find_first("path")
            .or_else(|| wire_expr.find_first("polyline_path"));
        if let Some(path_expr) = path_expr {
            let atoms = path_expr.atom_children();
            if atoms.len() >= 2 {
                let layer = atoms[0].to_string();
                let width = parse_f64(atoms[1]);
                let path = parse_coords(&atoms[2..]);
                let net = wire_expr
                    .find_first("net")
                    .and_then(|n| n.atom_children().first().copied())
                    .map(String::from);
                wiring.wires.push(Wire { layer, width, path, net });
            }
        }
    }

    for via_expr in expr.find_all("via") {
        let atoms = via_expr.atom_children();
        if atoms.len() >= 3 {
            let padstack = atoms[0].to_string();
            let x = parse_f64(atoms[1]);
            let y = parse_f64(atoms[2]);
            let net = via_expr
                .find_first("net")
                .and_then(|n| n.atom_children().first().copied())
                .map(String::from);
            wiring.vias.push(PlacedVia { padstack, x, y, net });
        }
    }

    wiring
}

// ─── Public entry point ──────────────────────────────────────────────────────

/// Preprocess DSN text to neutralise the `(string_quote ")` directive,
/// which would break the quoted-string grammar rule.
fn preprocess(input: &str) -> String {
    input.replace("(string_quote \")", "(string_quote dquote)")
}

pub fn parse_dsn(input: &str) -> anyhow::Result<Pcb> {
    let input = preprocess(input);
    let mut pairs = DsnParser::parse(Rule::file, &input)?;

    // `file` is the outer pair; its children are expr* items
    let file_pair = pairs.next().ok_or_else(|| anyhow::anyhow!("empty input"))?;
    let root_sexprs: Vec<SExpr> = file_pair.into_inner().filter_map(convert_pair).collect();

    let root = root_sexprs
        .into_iter()
        .find(|e| matches!(e.tag(), "pcb" | "PCB"))
        .ok_or_else(|| anyhow::anyhow!("no (pcb ...) root found"))?;

    let mut pcb = Pcb::default();
    if let SExpr::List(items) = &root {
        // items[0] = "pcb" tag, items[1] might be filename (atom)
        for item in &items[1..] {
            match item {
                SExpr::Atom(s) if pcb.id.is_empty() => pcb.id = s.clone(),
                SExpr::List(_) => match item.tag() {
                    "resolution" => {
                        let atoms = item.atom_children();
                        pcb.resolution_unit =
                            atoms.first().copied().unwrap_or("um").to_string();
                        pcb.resolution_value =
                            atoms.get(1).map(|s| parse_f64(s)).unwrap_or(1.0);
                    }
                    "unit" => {
                        pcb.unit = item
                            .atom_children()
                            .first()
                            .copied()
                            .unwrap_or("")
                            .to_string();
                    }
                    "structure" => pcb.structure = parse_structure(item),
                    "placement" => pcb.placement = parse_placement(item),
                    "library" => pcb.library = parse_library(item),
                    "network" => pcb.network = parse_network(item),
                    "wiring" => pcb.wiring = parse_wiring(item),
                    _ => {}
                },
                _ => {}
            }
        }
    }

    Ok(pcb)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[tracing_test::traced_test]
    fn test_parse_atoms() {
        pest::set_error_detail(true);
        for atom in [
            r#"(string_quote ")"#,
            r#"(comment "")"#,
            r#"(abc 123)"#,
            r#"(abc -123)"#,
            r#"(abc-1# -123)"#,
            r#"(MC-BD/R# -1.23)"#,
            r#"(host_version "(5.1.5)-3")"#,
            r#"(host_cad "KiCad's cad")"#,
        ] {
            DsnParser::parse(Rule::sexpr, atom)
                .unwrap_or_else(|e| panic!("failed to parse {atom:?}: {e}"));
        }
    }

    #[test]
    #[tracing_test::traced_test]
    fn test_parse_simple_dsn() {
        let pcb = parse_dsn(
            r#"(pcb ./test.dsn
              (resolution um 10)
              (unit um)
              (structure
                (layer F.Cu (type signal) (property (index 0)))
                (layer B.Cu (type signal) (property (index 1)))
                (boundary (path pcb 0 -5000 -5000 5000 -5000 5000 5000 -5000 5000 -5000 -5000))
                (via "Via[0-1]_600:300_um")
                (rule (width 200) (clearance 200))
              )
              (placement
                (component "Resistor_SMD:R_0402"
                  (place R1 3000 0 front 0 (PN "1k"))
                )
              )
              (library
                (image "Resistor_SMD:R_0402"
                  (pin padstack1 1 -500 0)
                  (pin padstack1 2  500 0)
                )
                (padstack "Via[0-1]_600:300_um"
                  (shape (circle F.Cu 600))
                  (shape (circle B.Cu 600))
                  (attach off)
                )
              )
              (network
                (net "Net1" (pins R1-1 C1-1))
                (class "default" "" "Net1"
                  (circuit (use_via "Via[0-1]_600:300_um"))
                  (rule (width 200) (clearance 200))
                )
              )
              (wiring
                (wire (path F.Cu 160 2500 0 -1200 0) (net "Net1") (type route))
              )
            )"#,
        )
        .expect("parse failed");

        assert_eq!(pcb.structure.layers.len(), 2);
        assert_eq!(pcb.structure.layers[0].name, "F.Cu");
        assert!(pcb.structure.boundary.is_some());
        assert_eq!(pcb.placement.components.len(), 1);
        assert_eq!(pcb.placement.components[0].places[0].id, "R1");
        assert_eq!(pcb.network.nets.len(), 1);
        assert_eq!(pcb.network.nets[0].name, "Net1");
        assert_eq!(pcb.wiring.wires.len(), 1);
    }
}

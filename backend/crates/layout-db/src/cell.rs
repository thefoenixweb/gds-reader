use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Polygon {
        layer: u16,
        datatype: u16,
        points: Vec<(f64, f64)>,
    },
    Path {
        layer: u16,
        datatype: u16,
        points: Vec<(f64, f64)>,
        width: f64,
        path_type: u8,
    },
    Text {
        layer: u16,
        texttype: u16,
        position: (f64, f64),
        string: String,
        rotation_deg: f64,
        magnification: f64,
        mirror_x: bool,
    },
}

impl Shape {
    pub fn bounding_box(&self) -> BoundingBox {
        match self {
            Shape::Polygon { points, .. } | Shape::Path { points, .. } => {
                BoundingBox::from_points(points)
            }
            Shape::Text { position, .. } => BoundingBox {
                x1: position.0,
                y1: position.1,
                x2: position.0,
                y2: position.1,
            },
        }
    }

    pub fn layer(&self) -> u16 {
        match self {
            Shape::Polygon { layer, .. } => *layer,
            Shape::Path    { layer, .. } => *layer,
            Shape::Text    { layer, .. } => *layer,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoundingBox {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl BoundingBox {
    pub fn from_points(points: &[(f64, f64)]) -> Self {
        assert!(!points.is_empty());
        let mut x1 = points[0].0;
        let mut y1 = points[0].1;
        let mut x2 = x1;
        let mut y2 = y1;
        for &(x, y) in points.iter().skip(1) {
            if x < x1 { x1 = x; }
            if y < y1 { y1 = y; }
            if x > x2 { x2 = x; }
            if y > y2 { y2 = y; }
        }
        BoundingBox { x1, y1, x2, y2 }
    }

    pub fn overlaps(&self, other: &BoundingBox) -> bool {
        self.x1 <= other.x2
            && self.x2 >= other.x1
            && self.y1 <= other.y2
            && self.y2 >= other.y1
    }

    pub fn union(&self, other: &BoundingBox) -> BoundingBox {
        BoundingBox {
            x1: self.x1.min(other.x1),
            y1: self.y1.min(other.y1),
            x2: self.x2.max(other.x2),
            y2: self.y2.max(other.y2),
        }
    }

    pub fn width(&self) -> f64  { self.x2 - self.x1 }
    pub fn height(&self) -> f64 { self.y2 - self.y1 }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SrefInstance {
    pub cell_name: String,
    pub position: (f64, f64),
    pub rotation_deg: f64,
    pub magnification: f64,
    pub mirror_x: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArefInstance {
    pub cell_name: String,
    pub origin: (f64, f64),
    pub rotation_deg: f64,
    pub magnification: f64,
    pub mirror_x: bool,
    pub cols: u16,
    pub rows: u16,
    pub col_vector: (f64, f64),
    pub row_vector: (f64, f64),
}

#[derive(Debug, Clone)]
pub struct Cell {
    pub name: String,
    pub shapes: Vec<Shape>,
    pub srefs: Vec<SrefInstance>,
    pub arefs: Vec<ArefInstance>,
}

impl Cell {
    pub fn new(name: impl Into<String>) -> Self {
        Cell { name: name.into(), shapes: Vec::new(), srefs: Vec::new(), arefs: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.shapes.is_empty() && self.srefs.is_empty() && self.arefs.is_empty()
    }

    pub fn direct_dependencies(&self) -> Vec<&str> {
        let mut deps: Vec<&str> = self.srefs.iter().map(|s| s.cell_name.as_str()).collect();
        deps.extend(self.arefs.iter().map(|a| a.cell_name.as_str()));
        deps.sort();
        deps.dedup();
        deps
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayerInfo {
    pub id: u16,
    pub name: String,
    pub color: String,
    pub visible: bool,
}

impl LayerInfo {
    pub fn new(id: u16) -> Self {
        LayerInfo {
            id,
            name: format!("Layer {id}"),
            color: Self::color_for_id(id),
            visible: true,
        }
    }

    fn color_for_id(id: u16) -> String {
        const PALETTE: &[&str] = &[
            "#3a86ff", "#ff006e", "#8338ec", "#fb5607",
            "#ffbe0b", "#06d6a0", "#118ab2", "#ef233c",
            "#b5179e", "#4cc9f0", "#4361ee", "#f72585",
        ];
        PALETTE[(id as usize) % PALETTE.len()].to_string()
    }
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub lib_name: String,
    pub cells: HashMap<String, Cell>,
    pub layers: HashMap<u16, LayerInfo>,
    pub top_cell: Option<String>,
    pub db_unit_in_meters: f64,
    pub user_unit_in_meters: f64,
}

impl Layout {
    pub fn new(lib_name: impl Into<String>) -> Self {
        Layout {
            lib_name: lib_name.into(),
            cells: HashMap::new(),
            layers: HashMap::new(),
            top_cell: None,
            db_unit_in_meters: 1e-9,
            user_unit_in_meters: 1e-6,
        }
    }

    pub fn register_layer(&mut self, id: u16) {
        self.layers.entry(id).or_insert_with(|| LayerInfo::new(id));
    }

    pub fn insert_cell(&mut self, cell: Cell) {
        for shape in &cell.shapes {
            self.register_layer(shape.layer());
        }
        self.cells.insert(cell.name.clone(), cell);
    }

    pub fn find_top_cell(&mut self) -> Option<&str> {
        let mut referenced: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for cell in self.cells.values() {
            for sref in &cell.srefs {
                referenced.insert(sref.cell_name.as_str());
            }
            for aref in &cell.arefs {
                referenced.insert(aref.cell_name.as_str());
            }
        }
        let top = self.cells.keys()
            .find(|name| !referenced.contains(name.as_str()))
            .cloned();
        self.top_cell = top;
        self.top_cell.as_deref()
    }

    pub fn to_microns(&self, db_value: f64) -> f64 {
        db_value * self.db_unit_in_meters / 1e-6
    }

    pub fn flat_shape_count(&self) -> usize {
        self.cells.values().map(|c| c.shapes.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_bounding_box_is_correct() {
        let shape = Shape::Polygon {
            layer: 1, datatype: 0,
            points: vec![(0.0, 0.0), (10.0, 0.0), (10.0, 5.0), (0.0, 5.0), (0.0, 0.0)],
        };
        let bb = shape.bounding_box();
        assert_eq!(bb.x1, 0.0);
        assert_eq!(bb.y1, 0.0);
        assert_eq!(bb.x2, 10.0);
        assert_eq!(bb.y2, 5.0);
    }

    #[test]
    fn text_bounding_box_is_point_at_origin() {
        let shape = Shape::Text {
            layer: 0, texttype: 0, position: (3.0, 7.0),
            string: "hello".to_string(), rotation_deg: 0.0,
            magnification: 1.0, mirror_x: false,
        };
        let bb = shape.bounding_box();
        assert_eq!(bb.x1, 3.0);
        assert_eq!(bb.y1, 7.0);
        assert_eq!(bb.x2, 3.0);
        assert_eq!(bb.y2, 7.0);
    }

    #[test]
    fn shape_layer_returns_correct_id() {
        let poly = Shape::Polygon { layer: 5, datatype: 0, points: vec![(0.,0.),(1.,0.),(1.,1.),(0.,1.),(0.,0.)] };
        let path = Shape::Path   { layer: 3, datatype: 0, points: vec![(0.,0.),(1.,1.)], width: 0.1, path_type: 0 };
        let text = Shape::Text   { layer: 9, texttype: 0, position: (0.,0.), string: "x".into(), rotation_deg: 0., magnification: 1., mirror_x: false };
        assert_eq!(poly.layer(), 5);
        assert_eq!(path.layer(), 3);
        assert_eq!(text.layer(), 9);
    }

    #[test]
    fn bounding_box_overlap_detects_intersection() {
        let a = BoundingBox { x1: 0.0, y1: 0.0, x2: 10.0, y2: 10.0 };
        let b = BoundingBox { x1: 5.0, y1: 5.0, x2: 15.0, y2: 15.0 };
        let c = BoundingBox { x1: 20.0, y1: 20.0, x2: 30.0, y2: 30.0 };
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn bounding_box_touching_edges_count_as_overlap() {
        let a = BoundingBox { x1: 0.0, y1: 0.0, x2: 10.0, y2: 10.0 };
        let b = BoundingBox { x1: 10.0, y1: 0.0, x2: 20.0, y2: 10.0 };
        assert!(a.overlaps(&b));
    }

    #[test]
    fn bounding_box_union_encompasses_both() {
        let a = BoundingBox { x1: 0.0, y1: 0.0, x2: 5.0, y2: 5.0 };
        let b = BoundingBox { x1: 3.0, y1: 3.0, x2: 10.0, y2: 10.0 };
        let u = a.union(&b);
        assert_eq!(u, BoundingBox { x1: 0.0, y1: 0.0, x2: 10.0, y2: 10.0 });
    }

    #[test]
    fn new_cell_is_empty() {
        let c = Cell::new("UNIT");
        assert!(c.is_empty());
        assert_eq!(c.name, "UNIT");
    }

    #[test]
    fn cell_direct_dependencies_deduplicates() {
        let mut cell = Cell::new("TOP");
        let sref = SrefInstance { cell_name: "BLOCK".to_string(), position: (0.0, 0.0), rotation_deg: 0.0, magnification: 1.0, mirror_x: false };
        cell.srefs.push(sref.clone());
        cell.srefs.push(sref);
        let deps = cell.direct_dependencies();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0], "BLOCK");
    }

    #[test]
    fn layer_info_default_name_matches_id() {
        let layer = LayerInfo::new(7);
        assert_eq!(layer.name, "Layer 7");
        assert!(layer.visible);
        assert!(layer.color.starts_with('#'));
        assert_eq!(layer.color.len(), 7);
    }

    #[test]
    fn layer_colors_differ_across_ids() {
        assert_ne!(LayerInfo::new(0).color, LayerInfo::new(1).color);
    }

    #[test]
    fn find_top_cell_identifies_unreferenced_root() {
        let mut layout = Layout::new("TEST_LIB");
        layout.insert_cell(Cell::new("UNIT"));
        let mut block = Cell::new("BLOCK");
        block.srefs.push(SrefInstance { cell_name: "UNIT".to_string(), position: (0.0, 0.0), rotation_deg: 0.0, magnification: 1.0, mirror_x: false });
        layout.insert_cell(block);
        let mut top = Cell::new("TOP");
        top.srefs.push(SrefInstance { cell_name: "BLOCK".to_string(), position: (0.0, 0.0), rotation_deg: 0.0, magnification: 1.0, mirror_x: false });
        layout.insert_cell(top);
        assert_eq!(layout.find_top_cell(), Some("TOP"));
    }

    #[test]
    fn flat_shape_count_sums_across_cells() {
        let mut layout = Layout::new("TEST_LIB");
        let mut cell_a = Cell::new("A");
        cell_a.shapes.push(Shape::Polygon { layer: 1, datatype: 0, points: vec![(0.,0.),(1.,0.),(1.,1.),(0.,1.),(0.,0.)] });
        cell_a.shapes.push(Shape::Polygon { layer: 1, datatype: 0, points: vec![(2.,2.),(3.,2.),(3.,3.),(2.,3.),(2.,2.)] });
        let mut cell_b = Cell::new("B");
        cell_b.shapes.push(Shape::Path { layer: 2, datatype: 0, points: vec![(0.,0.),(5.,5.)], width: 0.5, path_type: 0 });
        layout.insert_cell(cell_a);
        layout.insert_cell(cell_b);
        assert_eq!(layout.flat_shape_count(), 3);
    }

    #[test]
    fn register_layer_auto_creates_info() {
        let mut layout = Layout::new("TEST_LIB");
        layout.register_layer(42);
        assert!(layout.layers.contains_key(&42));
        assert_eq!(layout.layers[&42].id, 42);
    }

    #[test]
    fn to_microns_conversion_is_correct() {
        let mut layout = Layout::new("TEST_LIB");
        layout.db_unit_in_meters = 1e-9;
        assert!((layout.to_microns(1000.0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn layout_is_clone_and_debug() {
        let layout = Layout::new("CLONE_TEST");
        let cloned = layout.clone();
        assert_eq!(cloned.lib_name, "CLONE_TEST");
        let _ = format!("{:?}", cloned);
    }
}

use rstar::{RTree, RTreeObject, AABB};
use crate::cell::{BoundingBox, Layout, Shape};
use std::collections::HashMap;

pub struct IndexedShape {
    pub shape: Shape,
    pub cell_name: String,
    envelope: AABB<[f64; 2]>,
}

impl IndexedShape {
    pub fn new(shape: Shape, cell_name: String) -> Self {
        let bb = shape.bounding_box();
        let envelope = AABB::from_corners([bb.x1, bb.y1], [bb.x2, bb.y2]);
        IndexedShape { shape, cell_name, envelope }
    }
}

impl RTreeObject for IndexedShape {
    type Envelope = AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope { self.envelope }
}

#[derive(Clone, Copy)]
pub struct Transform {
    m11: f64, m12: f64, tx: f64,
    m21: f64, m22: f64, ty: f64,
}

impl Transform {
    pub fn identity() -> Self {
        Transform { m11: 1., m12: 0., tx: 0., m21: 0., m22: 1., ty: 0. }
    }

    pub fn from_gds(pos: (f64, f64), rot_deg: f64, mag: f64, mirror_x: bool) -> Self {
        let rad = rot_deg.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        let my = if mirror_x { -1.0 } else { 1.0 };
        
        Transform {
            m11: mag * cos,
            m12: -my * mag * sin,
            tx: pos.0,
            m21: mag * sin,
            m22: my * mag * cos,
            ty: pos.1,
        }
    }

    pub fn multiply(&self, other: &Transform) -> Transform {
        Transform {
            m11: self.m11 * other.m11 + self.m12 * other.m21,
            m12: self.m11 * other.m12 + self.m12 * other.m22,
            tx:  self.m11 * other.tx  + self.m12 * other.ty + self.tx,
            m21: self.m21 * other.m11 + self.m22 * other.m21,
            m22: self.m21 * other.m12 + self.m22 * other.m22,
            ty:  self.m21 * other.tx  + self.m22 * other.ty + self.ty,
        }
    }

    pub fn apply(&self, pt: (f64, f64)) -> (f64, f64) {
        (
            self.m11 * pt.0 + self.m12 * pt.1 + self.tx,
            self.m21 * pt.0 + self.m22 * pt.1 + self.ty
        )
    }

    pub fn inverse(&self) -> Self {
        let det = self.m11 * self.m22 - self.m12 * self.m21;
        if det.abs() < 1e-10 {
            return Transform::identity();
        }
        let inv = 1.0 / det;
        let m11 = self.m22 * inv;
        let m12 = -self.m12 * inv;
        let m21 = -self.m21 * inv;
        let m22 = self.m11 * inv;
        let tx = -(m11 * self.tx + m12 * self.ty);
        let ty = -(m21 * self.tx + m22 * self.ty);
        Transform { m11, m12, m21, m22, tx, ty }
    }
}

pub fn transform_shape(shape: &Shape, t: &Transform) -> Shape {
    match shape {
        Shape::Polygon { layer, datatype, points } => {
            let pts = points.iter().map(|&p| t.apply(p)).collect();
            Shape::Polygon { layer: *layer, datatype: *datatype, points: pts }
        }
        Shape::Path { layer, datatype, points, width, path_type } => {
            let pts = points.iter().map(|&p| t.apply(p)).collect();
            let mag = (t.m11 * t.m11 + t.m21 * t.m21).sqrt();
            Shape::Path { layer: *layer, datatype: *datatype, points: pts, width: width * mag, path_type: *path_type }
        }
        Shape::Text { layer, texttype, position, string, rotation_deg, magnification, mirror_x } => {
            let pos = t.apply(*position);
            Shape::Text { layer: *layer, texttype: *texttype, position: pos, string: string.clone(), rotation_deg: *rotation_deg, magnification: *magnification, mirror_x: *mirror_x }
        }
    }
}

pub struct IndexedInstance {
    pub cell_name: String,
    pub transform: Transform,
    envelope: AABB<[f64; 2]>,
}

impl RTreeObject for IndexedInstance {
    type Envelope = AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope { self.envelope }
}

pub struct CellIndex {
    pub shapes: RTree<IndexedShape>,
    pub instances: RTree<IndexedInstance>,
    pub bounding_box: Option<AABB<[f64; 2]>>,
}

pub struct ShapeIndex {
    cells: HashMap<String, CellIndex>,
    top_cell: Option<String>,
}

impl ShapeIndex {
    pub fn build_from_layout(layout: &Layout) -> Self {
        let mut cells = HashMap::new();
        let mut cell_bounds: HashMap<String, Option<AABB<[f64; 2]>>> = HashMap::new();

        let mut ordered_cells = Vec::new();
        let mut visited = std::collections::HashSet::new();
        
        fn visit(
            cell_name: &str, 
            layout: &Layout, 
            visited: &mut std::collections::HashSet<String>, 
            ordered: &mut Vec<String>
        ) {
            if visited.insert(cell_name.to_string()) {
                if let Some(cell) = layout.cells.get(cell_name) {
                    for dep in cell.direct_dependencies() {
                        visit(dep, layout, visited, ordered);
                    }
                }
                ordered.push(cell_name.to_string());
            }
        }
        
        if let Some(top) = &layout.top_cell {
            visit(top, layout, &mut visited, &mut ordered_cells);
        } else {
            for name in layout.cells.keys() {
                visit(name, layout, &mut visited, &mut ordered_cells);
            }
        }

        for cell_name in ordered_cells {
            if let Some(cell) = layout.cells.get(&cell_name) {
                let mut shape_entries = Vec::new();
                let mut inst_entries = Vec::new();
                let mut bb_min_x = f64::MAX;
                let mut bb_min_y = f64::MAX;
                let mut bb_max_x = f64::MIN;
                let mut bb_max_y = f64::MIN;
                
                let mut expand_bb = |x: f64, y: f64| {
                    if x < bb_min_x { bb_min_x = x; }
                    if y < bb_min_y { bb_min_y = y; }
                    if x > bb_max_x { bb_max_x = x; }
                    if y > bb_max_y { bb_max_y = y; }
                };

                for shape in &cell.shapes {
                    let indexed = IndexedShape::new(shape.clone(), cell_name.clone());
                    let env = indexed.envelope();
                    expand_bb(env.lower()[0], env.lower()[1]);
                    expand_bb(env.upper()[0], env.upper()[1]);
                    shape_entries.push(indexed);
                }
                
                let mut add_instance = |inst_cell: &str, transform: Transform| {
                    if let Some(Some(child_bb)) = cell_bounds.get(inst_cell) {
                        let c_min = child_bb.lower();
                        let c_max = child_bb.upper();
                        let pts = [
                            transform.apply((c_min[0], c_min[1])),
                            transform.apply((c_max[0], c_min[1])),
                            transform.apply((c_min[0], c_max[1])),
                            transform.apply((c_max[0], c_max[1])),
                        ];
                        let mut min_x = pts[0].0; let mut min_y = pts[0].1;
                        let mut max_x = pts[0].0; let mut max_y = pts[0].1;
                        for &(px, py) in &pts[1..] {
                            if px < min_x { min_x = px; }
                            if py < min_y { min_y = py; }
                            if px > max_x { max_x = px; }
                            if py > max_y { max_y = py; }
                        }
                        expand_bb(min_x, min_y);
                        expand_bb(max_x, max_y);
                        
                        let envelope = AABB::from_corners([min_x, min_y], [max_x, max_y]);
                        inst_entries.push(IndexedInstance {
                            cell_name: inst_cell.to_string(),
                            transform,
                            envelope,
                        });
                    }
                };

                for sref in &cell.srefs {
                    let t = Transform::from_gds(sref.position, sref.rotation_deg, sref.magnification, sref.mirror_x);
                    add_instance(&sref.cell_name, t);
                }
                
                for aref in &cell.arefs {
                    let base_t = Transform::from_gds(aref.origin, aref.rotation_deg, aref.magnification, aref.mirror_x);
                    let spacing_x = if aref.cols > 1 { aref.col_vector.0 / (aref.cols as f64) } else { 0.0 };
                    let spacing_y = if aref.cols > 1 { aref.col_vector.1 / (aref.cols as f64) } else { 0.0 };
                    let row_spacing_x = if aref.rows > 1 { aref.row_vector.0 / (aref.rows as f64) } else { 0.0 };
                    let row_spacing_y = if aref.rows > 1 { aref.row_vector.1 / (aref.rows as f64) } else { 0.0 };
                    
                    for i in 0..aref.cols {
                        for j in 0..aref.rows {
                            let dx = (i as f64) * spacing_x + (j as f64) * row_spacing_x;
                            let dy = (i as f64) * spacing_y + (j as f64) * row_spacing_y;
                            let mut t = base_t.clone();
                            t.tx += dx;
                            t.ty += dy;
                            add_instance(&aref.cell_name, t);
                        }
                    }
                }
                
                let bounding_box = if bb_min_x <= bb_max_x {
                    Some(AABB::from_corners([bb_min_x, bb_min_y], [bb_max_x, bb_max_y]))
                } else {
                    None
                };
                
                cell_bounds.insert(cell_name.clone(), bounding_box.clone());
                
                cells.insert(cell_name.clone(), CellIndex {
                    shapes: RTree::bulk_load(shape_entries),
                    instances: RTree::bulk_load(inst_entries),
                    bounding_box,
                });
            }
        }
        
        ShapeIndex { cells, top_cell: layout.top_cell.clone() }
    }

    pub fn query(&self, viewport: &BoundingBox, min_size: f64) -> Vec<IndexedShape> {
        let mut results = Vec::new();
        if let Some(top_name) = &self.top_cell {
            let env = AABB::from_corners([viewport.x1, viewport.y1], [viewport.x2, viewport.y2]);
            self.query_recursive(top_name, env, Transform::identity(), min_size, &mut results);
        } else {
            let env = AABB::from_corners([viewport.x1, viewport.y1], [viewport.x2, viewport.y2]);
            for (name, cell_idx) in &self.cells {
                for shape in cell_idx.shapes.locate_in_envelope_intersecting(&env) {
                    results.push(IndexedShape::new(shape.shape.clone(), name.clone()));
                }
            }
        }
        results
    }
    
    fn query_recursive(&self, cell_name: &str, viewport: AABB<[f64; 2]>, transform: Transform, min_size: f64, results: &mut Vec<IndexedShape>) {
        if let Some(cell_idx) = self.cells.get(cell_name) {
            let mag = (transform.m11 * transform.m11 + transform.m21 * transform.m21).sqrt();

            for indexed_shape in cell_idx.shapes.locate_in_envelope_intersecting(&viewport) {
                let env = indexed_shape.envelope();
                let w = env.upper()[0] - env.lower()[0];
                let h = env.upper()[1] - env.lower()[1];
                if w * mag < min_size && h * mag < min_size {
                    continue;
                }
                
                let tf_shape = transform_shape(&indexed_shape.shape, &transform);
                results.push(IndexedShape::new(tf_shape, cell_name.to_string()));
            }
            
            for instance in cell_idx.instances.locate_in_envelope_intersecting(&viewport) {
                let env = instance.envelope();
                let w = env.upper()[0] - env.lower()[0];
                let h = env.upper()[1] - env.lower()[1];
                
                if w * mag < min_size && h * mag < min_size {
                    // Too small to recurse, return a bounding box instead
                    let c_min = env.lower();
                    let c_max = env.upper();
                    let pts = vec![
                        (c_min[0], c_min[1]),
                        (c_max[0], c_min[1]),
                        (c_max[0], c_max[1]),
                        (c_min[0], c_max[1]),
                        (c_min[0], c_min[1]),
                    ];
                    // Layer 999 for LOD bounding boxes
                    let outline = Shape::Polygon { layer: 999, datatype: 0, points: pts };
                    let tf_shape = transform_shape(&outline, &transform);
                    results.push(IndexedShape::new(tf_shape, instance.cell_name.clone()));
                    continue;
                }

                let child_inv = instance.transform.inverse();
                let vp_min = viewport.lower();
                let vp_max = viewport.upper();
                let pts = [
                    child_inv.apply((vp_min[0], vp_min[1])),
                    child_inv.apply((vp_max[0], vp_min[1])),
                    child_inv.apply((vp_min[0], vp_max[1])),
                    child_inv.apply((vp_max[0], vp_max[1])),
                ];
                let mut min_x = pts[0].0; let mut min_y = pts[0].1;
                let mut max_x = pts[0].0; let mut max_y = pts[0].1;
                for &(px, py) in &pts[1..] {
                    if px < min_x { min_x = px; }
                    if py < min_y { min_y = py; }
                    if px > max_x { max_x = px; }
                    if py > max_y { max_y = py; }
                }
                let child_vp = AABB::from_corners([min_x, min_y], [max_x, max_y]);
                
                let child_t = transform.multiply(&instance.transform);
                self.query_recursive(&instance.cell_name, child_vp, child_t, min_size, results);
            }
        }
    }

    pub fn shape_count(&self) -> usize {
        self.cells.values().map(|c| c.shapes.size()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::{Cell, Layout, Shape};

    fn make_layout() -> Layout {
        let mut layout = Layout::new("TEST");
        let mut cell_a = Cell::new("A");
        cell_a.shapes.push(Shape::Polygon {
            layer: 1, datatype: 0,
            points: vec![(0.,0.),(10.,0.),(10.,10.),(0.,10.),(0.,0.)],
        });
        cell_a.shapes.push(Shape::Polygon {
            layer: 1, datatype: 0,
            points: vec![(20.,20.),(30.,20.),(30.,30.),(20.,30.),(20.,20.)],
        });
        let mut cell_b = Cell::new("B");
        cell_b.shapes.push(Shape::Path {
            layer: 2, datatype: 0,
            points: vec![(50.,50.),(60.,60.)],
            width: 1.0, path_type: 0,
        });
        layout.insert_cell(cell_a);
        layout.insert_cell(cell_b);
        layout
    }

    #[test]
    fn shape_count_reflects_all_shapes() {
        let index = ShapeIndex::build_from_layout(&make_layout());
        assert_eq!(index.shape_count(), 3);
    }

    #[test]
    fn query_returns_shapes_in_viewport() {
        let index = ShapeIndex::build_from_layout(&make_layout());
        let vp = BoundingBox { x1: -5., y1: -5., x2: 15., y2: 15. };
        let results = index.query(&vp);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn query_returns_multiple_overlapping_shapes() {
        let index = ShapeIndex::build_from_layout(&make_layout());
        let vp = BoundingBox { x1: 0., y1: 0., x2: 35., y2: 35. };
        let results = index.query(&vp);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn query_returns_empty_for_far_viewport() {
        let index = ShapeIndex::build_from_layout(&make_layout());
        let vp = BoundingBox { x1: 200., y1: 200., x2: 300., y2: 300. };
        let results = index.query(&vp);
        assert_eq!(results.len(), 0);
    }
}

use rstar::{RTree, RTreeObject, AABB, SelectionFunction, Envelope};
use crate::cell::{BoundingBox, Layout, Shape};
use std::collections::HashMap;

#[derive(Clone)]
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

#[derive(Clone, Copy, Debug)]
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
    
    pub fn to_array(&self) -> [f64; 6] {
        [self.m11, self.m12, self.tx, self.m21, self.m22, self.ty]
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

#[derive(Clone)]
pub struct IndexedInstance {
    pub cell_name: String,
    pub transform: Transform,
    pub cols: u16,
    pub rows: u16,
    pub col_vector: [f64; 2],
    pub row_vector: [f64; 2],
    envelope: AABB<[f64; 2]>,
}

impl RTreeObject for IndexedInstance {
    type Envelope = AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope { self.envelope }
}

#[derive(Clone)]
pub struct SizeAndIntersectionFilter {
    pub env: AABB<[f64; 2]>,
    pub min_size: f64,
}

impl SelectionFunction<IndexedShape> for SizeAndIntersectionFilter {
    fn should_unpack_parent(&self, envelope: &AABB<[f64; 2]>) -> bool {
        if !self.env.intersects(envelope) { return false; }
        let w = envelope.upper()[0] - envelope.lower()[0];
        let h = envelope.upper()[1] - envelope.lower()[1];
        w >= self.min_size || h >= self.min_size
    }

    fn should_unpack_leaf(&self, leaf: &IndexedShape) -> bool {
        let envelope = leaf.envelope();
        if !self.env.intersects(&envelope) { return false; }
        let w = envelope.upper()[0] - envelope.lower()[0];
        let h = envelope.upper()[1] - envelope.lower()[1];
        w >= self.min_size || h >= self.min_size
    }
}

impl SelectionFunction<IndexedInstance> for SizeAndIntersectionFilter {
    fn should_unpack_parent(&self, envelope: &AABB<[f64; 2]>) -> bool {
        if !self.env.intersects(envelope) { return false; }
        let w = envelope.upper()[0] - envelope.lower()[0];
        let h = envelope.upper()[1] - envelope.lower()[1];
        w >= self.min_size || h >= self.min_size
    }

    fn should_unpack_leaf(&self, leaf: &IndexedInstance) -> bool {
        let envelope = leaf.envelope();
        if !self.env.intersects(&envelope) { return false; }
        let w = envelope.upper()[0] - envelope.lower()[0];
        let h = envelope.upper()[1] - envelope.lower()[1];
        w >= self.min_size || h >= self.min_size
    }
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
                
                let mut add_instance = |inst_cell: &str, transform: Transform, cols: u16, rows: u16, col_vec: [f64; 2], row_vec: [f64; 2]| {
                    if let Some(Some(child_bb)) = cell_bounds.get(inst_cell) {
                        let c_min = child_bb.lower();
                        let c_max = child_bb.upper();
                        
                        let mut min_x = f64::MAX; let mut min_y = f64::MAX;
                        let mut max_x = f64::MIN; let mut max_y = f64::MIN;
                        
                        let spacing_x = if cols > 1 { col_vec[0] / ((cols - 1) as f64) } else { 0.0 };
                        let spacing_y = if cols > 1 { col_vec[1] / ((cols - 1) as f64) } else { 0.0 };
                        let row_spacing_x = if rows > 1 { row_vec[0] / ((rows - 1) as f64) } else { 0.0 };
                        let row_spacing_y = if rows > 1 { row_vec[1] / ((rows - 1) as f64) } else { 0.0 };
                        
                        let corners = [(0, 0), (cols.max(1) - 1, 0), (0, rows.max(1) - 1), (cols.max(1) - 1, rows.max(1) - 1)];
                        for (i, j) in corners {
                            let dx = (i as f64) * spacing_x + (j as f64) * row_spacing_x;
                            let dy = (i as f64) * spacing_y + (j as f64) * row_spacing_y;
                            let mut t = transform.clone();
                            t.tx += dx;
                            t.ty += dy;
                            
                            let pts = [
                                t.apply((c_min[0], c_min[1])),
                                t.apply((c_max[0], c_min[1])),
                                t.apply((c_min[0], c_max[1])),
                                t.apply((c_max[0], c_max[1])),
                            ];
                            for &(px, py) in &pts {
                                if px < min_x { min_x = px; }
                                if py < min_y { min_y = py; }
                                if px > max_x { max_x = px; }
                                if py > max_y { max_y = py; }
                            }
                        }
                        
                        expand_bb(min_x, min_y);
                        expand_bb(max_x, max_y);
                        
                        let envelope = AABB::from_corners([min_x, min_y], [max_x, max_y]);
                        inst_entries.push(IndexedInstance {
                            cell_name: inst_cell.to_string(),
                            transform,
                            cols,
                            rows,
                            col_vector: col_vec,
                            row_vector: row_vec,
                            envelope,
                        });
                    }
                };

                for sref in &cell.srefs {
                    let t = Transform::from_gds(sref.position, sref.rotation_deg, sref.magnification, sref.mirror_x);
                    add_instance(&sref.cell_name, t, 1, 1, [0.0, 0.0], [0.0, 0.0]);
                }
                
                for aref in &cell.arefs {
                    let base_t = Transform::from_gds(aref.origin, aref.rotation_deg, aref.magnification, aref.mirror_x);
                    add_instance(&aref.cell_name, base_t, aref.cols, aref.rows, [aref.col_vector.0, aref.col_vector.1], [aref.row_vector.0, aref.row_vector.1]);
                }
                
                let bounding_box = if bb_min_x <= bb_max_x {
                    Some(AABB::from_corners([bb_min_x, bb_min_y], [bb_max_x, bb_max_y]))
                } else {
                    None
                };
                
                cell_bounds.insert(cell_name.clone(), bounding_box.clone());
                
                cells.insert(cell_name.clone(), CellIndex {
                    shapes: RTree::bulk_load(shape_entries),
                    instances: RTree::bulk_load(inst_entries.clone()),
                    bounding_box: bounding_box.clone(),
                });
                
                if let Some(top) = &layout.top_cell {
                    if *top == cell_name {
                        println!("Diagnostics for top cell '{}':", top);
                        println!("Bounding box: {:?}", bounding_box);
                        println!("Top-level instances: {}", inst_entries.len());
                        for (idx, inst) in inst_entries.iter().enumerate().take(10) {
                            println!("  Inst {}: cell={}, transform={:?}, env={:?}", idx, inst.cell_name, inst.transform, inst.envelope());
                        }
                    }
                }
            }
        }
        
        ShapeIndex { cells, top_cell: layout.top_cell.clone() }
    }
    
    pub fn query_instanced(&self, viewport: &BoundingBox, min_size: f64, max_shapes: Option<usize>) -> (Vec<IndexedShape>, Vec<IndexedInstance>) {
        let mut res_shapes = Vec::new();
        let mut res_instances = Vec::new();
        
        if let Some(top_name) = &self.top_cell {
            let env = AABB::from_corners([viewport.x1, viewport.y1], [viewport.x2, viewport.y2]);
            self.query_instanced_recursive(top_name, env, Transform::identity(), min_size, max_shapes, &mut res_shapes, &mut res_instances);
        }
        
        (res_shapes, res_instances)
    }

    fn query_instanced_recursive(
        &self, 
        cell_name: &str, 
        viewport: AABB<[f64; 2]>, 
        transform: Transform, 
        min_size: f64, 
        max_shapes: Option<usize>,
        res_shapes: &mut Vec<IndexedShape>, 
        res_instances: &mut Vec<IndexedInstance>
    ) {
        if let Some(cell_idx) = self.cells.get(cell_name) {
            let mag = (transform.m11 * transform.m11 + transform.m21 * transform.m21).sqrt();
            let local_min_size = min_size / mag.max(1e-9);

            // Calculate inverse transform to map viewport to local coordinate space
            let det = transform.m11 * transform.m22 - transform.m12 * transform.m21;
            if det.abs() < 1e-9 { return; }
            let inv_m11 = transform.m22 / det;
            let inv_m12 = -transform.m12 / det;
            let inv_tx = (transform.m12 * transform.ty - transform.m22 * transform.tx) / det;
            let inv_m21 = -transform.m21 / det;
            let inv_m22 = transform.m11 / det;
            let inv_ty = (transform.m21 * transform.tx - transform.m11 * transform.ty) / det;

            let pts = [
                [viewport.lower()[0], viewport.lower()[1]],
                [viewport.upper()[0], viewport.lower()[1]],
                [viewport.upper()[0], viewport.upper()[1]],
                [viewport.lower()[0], viewport.upper()[1]],
            ];
            let mut local_min_x = f64::MAX;
            let mut local_min_y = f64::MAX;
            let mut local_max_x = f64::MIN;
            let mut local_max_y = f64::MIN;
            for p in &pts {
                let x = inv_m11 * p[0] + inv_m12 * p[1] + inv_tx;
                let y = inv_m21 * p[0] + inv_m22 * p[1] + inv_ty;
                local_min_x = local_min_x.min(x);
                local_min_y = local_min_y.min(y);
                local_max_x = local_max_x.max(x);
                local_max_y = local_max_y.max(y);
            }
            let local_env = AABB::from_corners([local_min_x, local_min_y], [local_max_x, local_max_y]);
            let filter = SizeAndIntersectionFilter { env: local_env, min_size: local_min_size };

            for indexed_shape in cell_idx.shapes.locate_with_selection_function(filter.clone()) {
                if let Some(max) = max_shapes {
                    if res_shapes.len() + res_instances.len() >= max {
                        return;
                    }
                }
                
                // Return shape in GLOBAL coordinates
                use crate::cell::Shape;
                let mut new_shape = indexed_shape.shape.clone();
                match &mut new_shape {
                    Shape::Polygon { points, .. } | Shape::Path { points, .. } => {
                        for pt in points.iter_mut() {
                            let tx = transform.m11 * pt.0 + transform.m12 * pt.1 + transform.tx;
                            let ty = transform.m21 * pt.0 + transform.m22 * pt.1 + transform.ty;
                            pt.0 = tx;
                            pt.1 = ty;
                        }
                    },
                    Shape::Text { position, .. } => {
                        let tx = transform.m11 * position.0 + transform.m12 * position.1 + transform.tx;
                        let ty = transform.m21 * position.0 + transform.m22 * position.1 + transform.ty;
                        position.0 = tx;
                        position.1 = ty;
                    }
                }
                res_shapes.push(IndexedShape::new(new_shape, cell_name.to_string()));
            }

            for indexed_inst in cell_idx.instances.locate_with_selection_function(filter) {
                if let Some(max) = max_shapes {
                    if res_shapes.len() + res_instances.len() >= max {
                        return;
                    }
                }
                
                // Check if target cell is massive
                let mut is_massive = false;
                if let Some(target_idx) = self.cells.get(&indexed_inst.cell_name) {
                    if target_idx.shapes.size() + target_idx.instances.size() > 5000 {
                        is_massive = true;
                    }
                }
                
                // Also, if it's an array, it's harder to flatten recursively, so we only flatten SREFs (cols=1, rows=1)
                // If it's massive AND an SREF, we flatten.
                if is_massive && indexed_inst.cols <= 1 && indexed_inst.rows <= 1 {
                    let inst_t = indexed_inst.transform;
                    let combined_m11 = transform.m11 * inst_t.m11 + transform.m12 * inst_t.m21;
                    let combined_m12 = transform.m11 * inst_t.m12 + transform.m12 * inst_t.m22;
                    let combined_tx = transform.m11 * inst_t.tx + transform.m12 * inst_t.ty + transform.tx;
                    let combined_m21 = transform.m21 * inst_t.m11 + transform.m22 * inst_t.m21;
                    let combined_m22 = transform.m21 * inst_t.m12 + transform.m22 * inst_t.m22;
                    let combined_ty = transform.m21 * inst_t.tx + transform.m22 * inst_t.ty + transform.ty;
                    
                    let combined_t = Transform {
                        m11: combined_m11, m12: combined_m12, tx: combined_tx,
                        m21: combined_m21, m22: combined_m22, ty: combined_ty,
                    };
                    
                    self.query_instanced_recursive(&indexed_inst.cell_name, viewport, combined_t, min_size, max_shapes, res_shapes, res_instances);
                } else {
                    // Return instance in GLOBAL coordinates
                    let mut new_inst = indexed_inst.clone();
                    let inst_t = new_inst.transform;
                    new_inst.transform.m11 = transform.m11 * inst_t.m11 + transform.m12 * inst_t.m21;
                    new_inst.transform.m12 = transform.m11 * inst_t.m12 + transform.m12 * inst_t.m22;
                    new_inst.transform.tx = transform.m11 * inst_t.tx + transform.m12 * inst_t.ty + transform.tx;
                    new_inst.transform.m21 = transform.m21 * inst_t.m11 + transform.m22 * inst_t.m21;
                    new_inst.transform.m22 = transform.m21 * inst_t.m12 + transform.m22 * inst_t.m22;
                    new_inst.transform.ty = transform.m21 * inst_t.tx + transform.m22 * inst_t.ty + transform.ty;
                    
                    // Transform array vectors
                    let cx = new_inst.col_vector[0];
                    let cy = new_inst.col_vector[1];
                    new_inst.col_vector[0] = transform.m11 * cx + transform.m12 * cy;
                    new_inst.col_vector[1] = transform.m21 * cx + transform.m22 * cy;
                    
                    let rx = new_inst.row_vector[0];
                    let ry = new_inst.row_vector[1];
                    new_inst.row_vector[0] = transform.m11 * rx + transform.m12 * ry;
                    new_inst.row_vector[1] = transform.m21 * rx + transform.m22 * ry;
                    
                    res_instances.push(new_inst);
                }
            }
        }
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

    pub fn get_cell_bounds(&self, cell_name: &str) -> Option<AABB<[f64; 2]>> {
        self.cells.get(cell_name).and_then(|c| c.bounding_box)
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

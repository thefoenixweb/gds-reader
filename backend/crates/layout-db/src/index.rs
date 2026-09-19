use rstar::{RTree, RTreeObject, AABB};
use crate::cell::{BoundingBox, Layout, Shape};

pub struct IndexedShape {
    pub shape: Shape,
    pub cell_name: String,
    envelope: AABB<[f64; 2]>,
}

impl IndexedShape {
    fn new(shape: Shape, cell_name: String) -> Self {
        let bb = shape.bounding_box();
        let envelope = AABB::from_corners([bb.x1, bb.y1], [bb.x2, bb.y2]);
        IndexedShape { shape, cell_name, envelope }
    }
}

impl RTreeObject for IndexedShape {
    type Envelope = AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope { self.envelope }
}

pub struct ShapeIndex {
    tree: RTree<IndexedShape>,
}

impl ShapeIndex {
    pub fn build_from_layout(layout: &Layout) -> Self {
        let entries: Vec<IndexedShape> = layout.cells
            .values()
            .flat_map(|cell| {
                cell.shapes.iter().map(|shape| {
                    IndexedShape::new(shape.clone(), cell.name.clone())
                })
            })
            .collect();
        ShapeIndex { tree: RTree::bulk_load(entries) }
    }

    pub fn query(&self, viewport: &BoundingBox) -> Vec<&IndexedShape> {
        let envelope = AABB::from_corners([viewport.x1, viewport.y1], [viewport.x2, viewport.y2]);
        self.tree.locate_in_envelope_intersecting(&envelope).collect()
    }

    pub fn shape_count(&self) -> usize {
        self.tree.size()
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
        assert!(results.is_empty());
    }

    #[test]
    fn query_result_carries_cell_name() {
        let index = ShapeIndex::build_from_layout(&make_layout());
        let vp = BoundingBox { x1: 45., y1: 45., x2: 65., y2: 65. };
        let results = index.query(&vp);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].cell_name, "B");
    }

    #[test]
    fn empty_layout_produces_empty_index() {
        let layout = Layout::new("EMPTY");
        let index = ShapeIndex::build_from_layout(&layout);
        assert_eq!(index.shape_count(), 0);
        let vp = BoundingBox { x1: 0., y1: 0., x2: 100., y2: 100. };
        assert!(index.query(&vp).is_empty());
    }

    #[test]
    fn text_shape_queryable_at_its_point() {
        let mut layout = Layout::new("T");
        let mut cell = Cell::new("C");
        cell.shapes.push(Shape::Text {
            layer: 0, texttype: 0,
            position: (5., 5.),
            string: "hi".into(),
            rotation_deg: 0., magnification: 1., mirror_x: false,
        });
        layout.insert_cell(cell);
        let index = ShapeIndex::build_from_layout(&layout);
        let vp = BoundingBox { x1: 0., y1: 0., x2: 10., y2: 10. };
        assert_eq!(index.query(&vp).len(), 1);
    }
}

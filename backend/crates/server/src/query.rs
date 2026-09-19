use serde::{Deserialize, Serialize};
use layout_db::cell::{BoundingBox, Shape};
use layout_db::index::ShapeIndex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewportQuery {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub zoom: f64,
    pub visible_layers: Vec<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LodLevel { Cell, Block, Shape }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ShapeDto {
    Polygon { id: u64, layer: u16, datatype: u16, points: Vec<[f64; 2]> },
    Path    { id: u64, layer: u16, datatype: u16, points: Vec<[f64; 2]>, width: f64, path_type: u8 },
    Text    { id: u64, layer: u16, texttype: u16, position: [f64; 2], string: String, rotation_deg: f64, magnification: f64, mirror_x: bool },
}

impl ShapeDto {
    pub fn id(&self) -> u64 {
        match self {
            ShapeDto::Polygon { id, .. } | ShapeDto::Path { id, .. } | ShapeDto::Text { id, .. } => *id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewportResponse {
    pub shapes: Vec<ShapeDto>,
    pub lod_level: LodLevel,
}

pub fn handle_query(index: &ShapeIndex, query: &ViewportQuery) -> ViewportResponse {
    let lod_level = match query.zoom {
        z if z < 1.0 => LodLevel::Cell,
        z if z < 5.0 => LodLevel::Block,
        _            => LodLevel::Shape,
    };

    let viewport = BoundingBox { x1: query.x1, y1: query.y1, x2: query.x2, y2: query.y2 };

    let shapes = index.query(&viewport)
        .into_iter()
        .enumerate()
        .filter(|(_, entry)| {
            query.visible_layers.is_empty()
                || query.visible_layers.contains(&entry.shape.layer())
        })
        .map(|(id, entry)| shape_to_dto(id as u64, &entry.shape))
        .collect();

    ViewportResponse { shapes, lod_level }
}

fn shape_to_dto(id: u64, shape: &Shape) -> ShapeDto {
    match shape {
        Shape::Polygon { layer, datatype, points } => ShapeDto::Polygon {
            id, layer: *layer, datatype: *datatype,
            points: points.iter().map(|&(x, y)| [x, y]).collect(),
        },
        Shape::Path { layer, datatype, points, width, path_type } => ShapeDto::Path {
            id, layer: *layer, datatype: *datatype,
            points: points.iter().map(|&(x, y)| [x, y]).collect(),
            width: *width, path_type: *path_type,
        },
        Shape::Text { layer, texttype, position, string, rotation_deg, magnification, mirror_x } => ShapeDto::Text {
            id, layer: *layer, texttype: *texttype,
            position: [position.0, position.1], string: string.clone(),
            rotation_deg: *rotation_deg, magnification: *magnification, mirror_x: *mirror_x,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layout_db::cell::{Cell, Layout, Shape};
    use layout_db::index::ShapeIndex;

    fn make_index() -> ShapeIndex {
        let mut layout = Layout::new("TEST");
        let mut cell = Cell::new("A");
        cell.shapes.push(Shape::Polygon {
            layer: 1, datatype: 0,
            points: vec![(0.,0.),(10.,0.),(10.,10.),(0.,10.),(0.,0.)],
        });
        cell.shapes.push(Shape::Polygon {
            layer: 2, datatype: 0,
            points: vec![(5.,5.),(15.,5.),(15.,15.),(5.,15.),(5.,5.)],
        });
        layout.insert_cell(cell);
        ShapeIndex::build_from_layout(&layout)
    }

    fn query(x1: f64, y1: f64, x2: f64, y2: f64, zoom: f64, layers: Vec<u16>) -> ViewportQuery {
        ViewportQuery { x1, y1, x2, y2, zoom, visible_layers: layers }
    }

    #[test]
    fn returns_shapes_in_viewport() {
        let resp = handle_query(&make_index(), &query(0.,0.,20.,20., 10.0, vec![]));
        assert_eq!(resp.shapes.len(), 2);
        assert_eq!(resp.lod_level, LodLevel::Shape);
    }

    #[test]
    fn layer_filter_excludes_hidden_layers() {
        let resp = handle_query(&make_index(), &query(0.,0.,20.,20., 10.0, vec![1]));
        assert_eq!(resp.shapes.len(), 1);
    }

    #[test]
    fn empty_visible_layers_returns_all() {
        let resp = handle_query(&make_index(), &query(0.,0.,20.,20., 10.0, vec![]));
        assert_eq!(resp.shapes.len(), 2);
    }

    #[test]
    fn lod_cell_at_low_zoom() {
        let resp = handle_query(&make_index(), &query(0.,0.,1000.,1000., 0.5, vec![]));
        assert_eq!(resp.lod_level, LodLevel::Cell);
    }

    #[test]
    fn lod_block_at_mid_zoom() {
        let resp = handle_query(&make_index(), &query(0.,0.,50.,50., 2.0, vec![]));
        assert_eq!(resp.lod_level, LodLevel::Block);
    }

    #[test]
    fn lod_shape_at_high_zoom() {
        let resp = handle_query(&make_index(), &query(0.,0.,20.,20., 5.0, vec![]));
        assert_eq!(resp.lod_level, LodLevel::Shape);
    }

    #[test]
    fn shape_dto_serializes_to_json() {
        let resp = handle_query(&make_index(), &query(0.,0.,20.,20., 10.0, vec![]));
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"kind\""));
        assert!(json.contains("\"lodLevel\""));
        assert!(json.contains("\"shape\""));
    }

    #[test]
    fn shape_dto_ids_are_unique() {
        let resp = handle_query(&make_index(), &query(0.,0.,20.,20., 10.0, vec![]));
        let mut ids: Vec<u64> = resp.shapes.iter().map(|s| s.id()).collect();
        let original_len = ids.len();
        ids.sort(); ids.dedup();
        assert_eq!(ids.len(), original_len);
    }

    #[test]
    fn viewport_query_roundtrips_json() {
        let q = query(1.0, 2.0, 100.0, 200.0, 8.0, vec![1, 2]);
        let json = serde_json::to_string(&q).unwrap();
        let q2: ViewportQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(q2.x1, 1.0);
        assert_eq!(q2.visible_layers, vec![1, 2]);
        assert!(json.contains("visibleLayers"));
    }
}

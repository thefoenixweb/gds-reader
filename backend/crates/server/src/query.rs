use serde::{Deserialize, Serialize};
use layout_db::cell::{BoundingBox, Shape, Layout};
use layout_db::index::{ShapeIndex, IndexedInstance, IndexedShape};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewportQuery {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub zoom: f64,
    pub visible_layers: Vec<u16>,
    #[serde(default)]
    pub known_cells: HashMap<String, LodLevel>,
    #[serde(default)]
    pub max_shapes: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
pub struct InstanceDto {
    pub cell_name: String,
    // [m11, m12, tx, m21, m22, ty]
    pub transform: [f64; 6],
    pub cols: u16,
    pub rows: u16,
    pub col_vector: [f64; 2],
    pub row_vector: [f64; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellDefinitionDto {
    pub bbox: [f64; 4],
    pub shapes: Vec<ShapeDto>,
    pub instances: Vec<InstanceDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewportResponse {
    pub shapes: Vec<ShapeDto>,
    pub instances: Vec<InstanceDto>,
    pub cell_definitions: HashMap<String, CellDefinitionDto>,
    pub lod_level: LodLevel,
}

impl ViewportResponse {
    pub fn to_binary(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        let lod = match self.lod_level {
            LodLevel::Cell => 0u8,
            LodLevel::Block => 1u8,
            LodLevel::Shape => 2u8,
        };
        buf.push(lod);
        
        // 1. Shapes
        let valid_shapes: Vec<&ShapeDto> = self.shapes.iter().filter(|s| !matches!(s, ShapeDto::Text { .. })).collect();
        buf.extend_from_slice(&(valid_shapes.len() as u32).to_le_bytes());
        for shape in valid_shapes {
            Self::encode_shape(shape, &mut buf);
        }
        
        // 2. Instances
        buf.extend_from_slice(&(self.instances.len() as u32).to_le_bytes());
        for inst in &self.instances {
            let name_bytes = inst.cell_name.as_bytes();
            buf.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
            buf.extend_from_slice(name_bytes);
            for i in 0..6 {
                buf.extend_from_slice(&inst.transform[i].to_le_bytes());
            }
            buf.extend_from_slice(&inst.cols.to_le_bytes());
            buf.extend_from_slice(&inst.rows.to_le_bytes());
            buf.extend_from_slice(&inst.col_vector[0].to_le_bytes());
            buf.extend_from_slice(&inst.col_vector[1].to_le_bytes());
            buf.extend_from_slice(&inst.row_vector[0].to_le_bytes());
            buf.extend_from_slice(&inst.row_vector[1].to_le_bytes());
        }
        
        // 3. Cell Definitions
        buf.extend_from_slice(&(self.cell_definitions.len() as u32).to_le_bytes());
        for (name, def) in &self.cell_definitions {
            let name_bytes = name.as_bytes();
            buf.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
            buf.extend_from_slice(name_bytes);
            
            // bbox
            for i in 0..4 {
                buf.extend_from_slice(&def.bbox[i].to_le_bytes());
            }
            
            let valid_def_shapes: Vec<&ShapeDto> = def.shapes.iter().filter(|s| !matches!(s, ShapeDto::Text { .. })).collect();
            buf.extend_from_slice(&(valid_def_shapes.len() as u32).to_le_bytes());
            for shape in valid_def_shapes {
                Self::encode_shape(shape, &mut buf);
            }
            
            buf.extend_from_slice(&(def.instances.len() as u32).to_le_bytes());
            for inst in &def.instances {
                let inst_name = inst.cell_name.as_bytes();
                buf.extend_from_slice(&(inst_name.len() as u32).to_le_bytes());
                buf.extend_from_slice(inst_name);
                for i in 0..6 {
                    buf.extend_from_slice(&inst.transform[i].to_le_bytes());
                }
                buf.extend_from_slice(&inst.cols.to_le_bytes());
                buf.extend_from_slice(&inst.rows.to_le_bytes());
                buf.extend_from_slice(&inst.col_vector[0].to_le_bytes());
                buf.extend_from_slice(&inst.col_vector[1].to_le_bytes());
                buf.extend_from_slice(&inst.row_vector[0].to_le_bytes());
                buf.extend_from_slice(&inst.row_vector[1].to_le_bytes());
            }
        }
        
        buf
    }
    
    fn encode_shape(shape: &ShapeDto, buf: &mut Vec<u8>) {
        match shape {
            ShapeDto::Polygon { layer, datatype, points, .. } => {
                buf.push(0u8);
                buf.extend_from_slice(&layer.to_le_bytes());
                buf.extend_from_slice(&datatype.to_le_bytes());
                buf.extend_from_slice(&(points.len() as u32).to_le_bytes());
                for pt in points {
                    buf.extend_from_slice(&pt[0].to_le_bytes());
                    buf.extend_from_slice(&pt[1].to_le_bytes());
                }
            },
            ShapeDto::Path { layer, datatype, points, width, path_type, .. } => {
                buf.push(1u8);
                buf.extend_from_slice(&layer.to_le_bytes());
                buf.extend_from_slice(&datatype.to_le_bytes());
                buf.extend_from_slice(&width.to_le_bytes());
                buf.push(*path_type);
                buf.extend_from_slice(&(points.len() as u32).to_le_bytes());
                for pt in points {
                    buf.extend_from_slice(&pt[0].to_le_bytes());
                    buf.extend_from_slice(&pt[1].to_le_bytes());
                }
            },
            ShapeDto::Text { .. } => {
                // Ignore text for now to keep binary simple
            }
        }
    }
}

pub fn handle_query(index: &ShapeIndex, layout: Option<&Layout>, query: &ViewportQuery) -> ViewportResponse {
    let lod_level = match query.zoom {
        z if z < 0.00001 => LodLevel::Cell,
        z if z < 0.0001 => LodLevel::Block,
        _            => LodLevel::Shape,
    };

    let viewport = BoundingBox { x1: query.x1, y1: query.y1, x2: query.x2, y2: query.y2 };
    let min_size_world = 3.0 / query.zoom.max(1e-9);

    let (raw_shapes, raw_instances) = index.query_instanced(&viewport, min_size_world, query.max_shapes);
    
    // Process shapes (top level)
    let shapes: Vec<ShapeDto> = raw_shapes
        .into_iter()
        .enumerate()
        .filter(|(_, entry)| {
            query.visible_layers.is_empty()
                || query.visible_layers.contains(&entry.shape.layer())
                || entry.shape.layer() == 999
        })
        .map(|(id, entry)| shape_to_dto(id as u64, &entry.shape))
        .collect();

    // Process instances
    let mut instances = Vec::new();
    let mut required_cells = HashSet::new();
    
    for inst in raw_instances.into_iter() {
        let spacing_x = if inst.cols > 1 { inst.col_vector[0] / ((inst.cols - 1) as f64) } else { 0.0 };
        let spacing_y = if inst.cols > 1 { inst.col_vector[1] / ((inst.cols - 1) as f64) } else { 0.0 };
        let row_spacing_x = if inst.rows > 1 { inst.row_vector[0] / ((inst.rows - 1) as f64) } else { 0.0 };
        let row_spacing_y = if inst.rows > 1 { inst.row_vector[1] / ((inst.rows - 1) as f64) } else { 0.0 };

        instances.push(InstanceDto {
            cell_name: inst.cell_name.clone(),
            transform: inst.transform.to_array(),
            cols: inst.cols,
            rows: inst.rows,
            col_vector: [spacing_x, spacing_y],
            row_vector: [row_spacing_x, row_spacing_y],
        });
        required_cells.insert(inst.cell_name);
    }
    
    let known: HashMap<&str, LodLevel> = query.known_cells.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    
    // Bundle missing cell definitions recursively
    let mut cell_definitions = HashMap::new();
    if let Some(l) = layout {
        let mut queue: Vec<String> = required_cells.into_iter().filter(|c| {
            match known.get(c.as_str()) {
                Some(&client_lod) => client_lod < lod_level,
                None => true,
            }
        }).collect();
        let mut processed = HashSet::new();
        
        while let Some(cell_name) = queue.pop() {
            if processed.insert(cell_name.clone()) {
                if let Some(cell) = l.cells.get(&cell_name) {
                    let mut shape_dtos: Vec<ShapeDto> = Vec::new();
                    
                    shape_dtos = cell.shapes.iter()
                        .enumerate()
                        .filter(|(_, s)| {
                            query.visible_layers.is_empty()
                                || query.visible_layers.contains(&s.layer())
                                || s.layer() == 999
                        })
                        .map(|(id, s)| shape_to_dto(id as u64, s))
                        .collect();
                        
                    let mut inst_dtos = Vec::new();
                    
                    for sref in &cell.srefs {
                        let t = layout_db::index::Transform::from_gds(sref.position, sref.rotation_deg, sref.magnification, sref.mirror_x).to_array();
                        inst_dtos.push(InstanceDto {
                            cell_name: sref.cell_name.clone(),
                            transform: t,
                            cols: 1,
                            rows: 1,
                            col_vector: [0.0, 0.0],
                            row_vector: [0.0, 0.0],
                        });
                        let needs_update = match known.get(sref.cell_name.as_str()) {
                            Some(&client_lod) => client_lod < lod_level,
                            None => true,
                        };
                        if needs_update && !processed.contains(&sref.cell_name) {
                            queue.push(sref.cell_name.clone());
                        }
                    }
                    for aref in &cell.arefs {
                        let base_t = layout_db::index::Transform::from_gds(aref.origin, aref.rotation_deg, aref.magnification, aref.mirror_x).to_array();
                        let spacing_x = if aref.cols > 1 { aref.col_vector.0 / ((aref.cols - 1) as f64) } else { 0.0 };
                        let spacing_y = if aref.cols > 1 { aref.col_vector.1 / ((aref.cols - 1) as f64) } else { 0.0 };
                        let row_spacing_x = if aref.rows > 1 { aref.row_vector.0 / ((aref.rows - 1) as f64) } else { 0.0 };
                        let row_spacing_y = if aref.rows > 1 { aref.row_vector.1 / ((aref.rows - 1) as f64) } else { 0.0 };
                        
                        inst_dtos.push(InstanceDto {
                            cell_name: aref.cell_name.clone(),
                            transform: base_t,
                            cols: aref.cols,
                            rows: aref.rows,
                            col_vector: [spacing_x, spacing_y],
                            row_vector: [row_spacing_x, row_spacing_y],
                        });
                        let needs_update = match known.get(aref.cell_name.as_str()) {
                            Some(&client_lod) => client_lod < lod_level,
                            None => true,
                        };
                        if needs_update && !processed.contains(&aref.cell_name) {
                            queue.push(aref.cell_name.clone());
                        }
                    }
                    
                    let bbox = index.get_cell_bounds(&cell_name).map(|env| {
                        [env.lower()[0], env.lower()[1], env.upper()[0], env.upper()[1]]
                    }).unwrap_or([0.0, 0.0, 0.0, 0.0]);
                    
                    cell_definitions.insert(cell_name.clone(), CellDefinitionDto {
                        bbox,
                        shapes: shape_dtos,
                        instances: inst_dtos,
                    });
                }
            }
        }
    }

    ViewportResponse { shapes, instances, cell_definitions, lod_level }
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
        ViewportQuery { x1, y1, x2, y2, zoom, visible_layers: layers, known_cells: HashMap::new() }
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
    fn test_handle_query_culling() {
        let index = make_index();
        let vp = BoundingBox { x1: -5., y1: -5., x2: 15., y2: 15. };
        let results = index.query(&vp, 0.0);
        assert_eq!(results.len(), 2);
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

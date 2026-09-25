use std::io::Read;
use layout_db::cell::{ArefInstance, Cell, Layout, Shape, SrefInstance};
use crate::reader::GdsReader;
use crate::types::{
    GdsError, GdsRecord,
    RT_LIBNAME, RT_SNAME, RT_STRING, RT_STRNAME,
};

// Decode helpers

fn decode_i16(d: &[u8]) -> i16 {
    i16::from_be_bytes([d[0], d[1]])
}

fn decode_u16(d: &[u8]) -> u16 {
    u16::from_be_bytes([d[0], d[1]])
}

fn decode_i32(d: &[u8]) -> i32 {
    i32::from_be_bytes([d[0], d[1], d[2], d[3]])
}

fn decode_xy(data: &[u8]) -> Vec<(f64, f64)> {
    data.chunks_exact(8)
        .map(|c| (decode_i32(&c[0..4]) as f64, decode_i32(&c[4..8]) as f64))
        .collect()
}

fn decode_string(data: &[u8], record_type: u8) -> Result<String, GdsError> {
    let end = data.iter().rposition(|&b| b != 0).map(|i| i + 1).unwrap_or(0);
    String::from_utf8(data[..end].to_vec())
        .map_err(|_| GdsError::InvalidAscii { record_type })
}

// IBM 360 8-byte floating point (big-endian, base-16 exponent, excess-64 bias).
pub fn decode_ibm_real8(bytes: &[u8]) -> f64 {
    let sign: f64 = if bytes[0] & 0x80 != 0 { -1.0 } else { 1.0 };
    let exponent = (bytes[0] & 0x7F) as i32 - 64;
    let mut mantissa: u64 = 0;
    for i in 1..8 {
        mantissa = (mantissa << 8) | (bytes[i] as u64);
    }
    let fraction = mantissa as f64 / (1u64 << 56) as f64;
    sign * fraction * 16f64.powi(exponent)
}

fn decode_strans_mirror(data: &[u8]) -> bool {
    data.len() >= 2 && (u16::from_be_bytes([data[0], data[1]]) & 0x8000) != 0
}

// ── Element builders ───────────────────────────────────────────────────────────

struct BoundaryBuilder { layer: u16, datatype: u16, points: Vec<(f64, f64)> }
struct PathBuilder     { layer: u16, datatype: u16, points: Vec<(f64, f64)>, width: f64, path_type: u8 }
struct SrefBuilder     { cell_name: String, position: (f64, f64), rotation_deg: f64, magnification: f64, mirror_x: bool }
struct ArefBuilder     { cell_name: String, origin: (f64, f64), rotation_deg: f64, magnification: f64, mirror_x: bool, cols: u16, rows: u16, col_vector: (f64, f64), row_vector: (f64, f64) }
struct TextBuilder     { layer: u16, texttype: u16, position: (f64, f64), string: String, rotation_deg: f64, magnification: f64, mirror_x: bool }

enum ParserState {
    Start,
    InLibrary,
    InCell,
    InBoundary(BoundaryBuilder),
    InPath(PathBuilder),
    InSref(SrefBuilder),
    InAref(ArefBuilder),
    InText(TextBuilder),
}

//  GdsParser 

pub struct GdsParser<R: Read> {
    reader: GdsReader<R>,
}

impl<R: Read> GdsParser<R> {
    pub fn new(reader: GdsReader<R>) -> Self {
        GdsParser { reader }
    }

    pub fn parse(mut self) -> Result<Layout, GdsError> {
        let mut layout = Layout::new("");
        let mut current_cell: Option<Cell> = None;
        let mut state = ParserState::Start;

        loop {
            let record = match self.reader.read_record() {
                Ok(r)                        => r,
                Err(GdsError::UnexpectedEof) => return Err(GdsError::UnexpectedEof),
                Err(e)                       => return Err(e),
            };

            match record {
                GdsRecord::EndLib => break,

                GdsRecord::Header(_) => {}

                GdsRecord::BgnLib(_) => { state = ParserState::InLibrary; }

                GdsRecord::LibName(data) => {
                    layout.lib_name = decode_string(&data, RT_LIBNAME)?;
                }

                GdsRecord::Units(data) if data.len() >= 16 => {
                    layout.db_unit_in_meters   = decode_ibm_real8(&data[8..16]);
                    layout.user_unit_in_meters = {
                        let user_per_db = decode_ibm_real8(&data[0..8]);
                        if user_per_db != 0.0 { layout.db_unit_in_meters / user_per_db } else { 1e-6 }
                    };
                }

                GdsRecord::BgnStr(_) => {
                    current_cell = Some(Cell::new(""));
                    state = ParserState::InCell;
                }

                GdsRecord::StrName(data) => {
                    if let Some(ref mut cell) = current_cell {
                        cell.name = decode_string(&data, RT_STRNAME)?;
                    }
                }

                GdsRecord::EndStr => {
                    if let Some(cell) = current_cell.take() {
                        layout.insert_cell(cell);
                    }
                    state = ParserState::InLibrary;
                }

                // Element openers
                GdsRecord::Boundary => { state = ParserState::InBoundary(BoundaryBuilder { layer: 0, datatype: 0, points: vec![] }); }
                GdsRecord::Path     => { state = ParserState::InPath(PathBuilder     { layer: 0, datatype: 0, points: vec![], width: 0.0, path_type: 0 }); }
                GdsRecord::Sref     => { state = ParserState::InSref(SrefBuilder     { cell_name: String::new(), position: (0.0, 0.0), rotation_deg: 0.0, magnification: 1.0, mirror_x: false }); }
                GdsRecord::Aref     => { state = ParserState::InAref(ArefBuilder     { cell_name: String::new(), origin: (0.0, 0.0), rotation_deg: 0.0, magnification: 1.0, mirror_x: false, cols: 1, rows: 1, col_vector: (0.0, 0.0), row_vector: (0.0, 0.0) }); }
                GdsRecord::Text     => { state = ParserState::InText(TextBuilder     { layer: 0, texttype: 0, position: (0.0, 0.0), string: String::new(), rotation_deg: 0.0, magnification: 1.0, mirror_x: false }); }

                GdsRecord::Layer(data) => {
                    let v = decode_i16(&data) as u16;
                    match &mut state {
                        ParserState::InBoundary(b) => b.layer = v,
                        ParserState::InPath(p)     => p.layer = v,
                        ParserState::InText(t)     => t.layer = v,
                        _ => {}
                    }
                }

                GdsRecord::DataType(data) => {
                    let v = decode_i16(&data) as u16;
                    match &mut state {
                        ParserState::InBoundary(b) => b.datatype = v,
                        ParserState::InPath(p)     => p.datatype = v,
                        _ => {}
                    }
                }

                GdsRecord::TextType(data) => {
                    if let ParserState::InText(t) = &mut state {
                        t.texttype = decode_i16(&data) as u16;
                    }
                }

                GdsRecord::Width(data) => {
                    if let ParserState::InPath(p) = &mut state {
                        p.width = decode_i32(&data) as f64;
                    }
                }

                GdsRecord::PathType(data) => {
                    if let ParserState::InPath(p) = &mut state {
                        p.path_type = decode_i16(&data).max(0) as u8;
                    }
                }

                GdsRecord::XY(data) => {
                    let pts = decode_xy(&data);
                    match &mut state {
                        ParserState::InBoundary(b) => b.points = pts,
                        ParserState::InPath(p)     => p.points = pts,
                        ParserState::InSref(s) => {
                            if let Some(&p) = pts.first() { s.position = p; }
                        }
                        ParserState::InAref(a) => {
                            if pts.len() >= 3 {
                                a.origin     = pts[0];
                                a.col_vector = (pts[1].0 - pts[0].0, pts[1].1 - pts[0].1);
                                a.row_vector = (pts[2].0 - pts[0].0, pts[2].1 - pts[0].1);
                            }
                        }
                        ParserState::InText(t) => {
                            if let Some(&p) = pts.first() { t.position = p; }
                        }
                        _ => {}
                    }
                }

                GdsRecord::SName(data) => {
                    let name = decode_string(&data, RT_SNAME)?;
                    match &mut state {
                        ParserState::InSref(s) => s.cell_name = name,
                        ParserState::InAref(a) => a.cell_name = name,
                        _ => {}
                    }
                }

                GdsRecord::ColRow(data) if data.len() >= 4 => {
                    if let ParserState::InAref(a) = &mut state {
                        a.cols = decode_u16(&data[0..2]).max(1);
                        a.rows = decode_u16(&data[2..4]).max(1);
                    }
                }

                GdsRecord::STrans(data) => {
                    let mirror = decode_strans_mirror(&data);
                    match &mut state {
                        ParserState::InSref(s) => s.mirror_x = mirror,
                        ParserState::InAref(a) => a.mirror_x = mirror,
                        ParserState::InText(t) => t.mirror_x = mirror,
                        _ => {}
                    }
                }

                GdsRecord::Mag(data) if data.len() >= 8 => {
                    let mag = decode_ibm_real8(&data);
                    match &mut state {
                        ParserState::InSref(s) => s.magnification = mag,
                        ParserState::InAref(a) => a.magnification = mag,
                        ParserState::InText(t) => t.magnification = mag,
                        _ => {}
                    }
                }

                GdsRecord::Angle(data) if data.len() >= 8 => {
                    let angle = decode_ibm_real8(&data);
                    match &mut state {
                        ParserState::InSref(s) => s.rotation_deg = angle,
                        ParserState::InAref(a) => a.rotation_deg = angle,
                        ParserState::InText(t) => t.rotation_deg = angle,
                        _ => {}
                    }
                }

                GdsRecord::String(data) => {
                    if let ParserState::InText(t) = &mut state {
                        t.string = decode_string(&data, RT_STRING)?;
                    }
                }

                GdsRecord::EndEl => {
                    let prev = std::mem::replace(&mut state, ParserState::InCell);
                    if let Some(ref mut cell) = current_cell {
                        match prev {
                            ParserState::InBoundary(b) => cell.shapes.push(Shape::Polygon { layer: b.layer, datatype: b.datatype, points: b.points }),
                            ParserState::InPath(p)     => cell.shapes.push(Shape::Path    { layer: p.layer, datatype: p.datatype, points: p.points, width: p.width, path_type: p.path_type }),
                            ParserState::InText(t)     => cell.shapes.push(Shape::Text    { layer: t.layer, texttype: t.texttype, position: t.position, string: t.string, rotation_deg: t.rotation_deg, magnification: t.magnification, mirror_x: t.mirror_x }),
                            ParserState::InSref(s)     => cell.srefs.push(SrefInstance    { cell_name: s.cell_name, position: s.position, rotation_deg: s.rotation_deg, magnification: s.magnification, mirror_x: s.mirror_x }),
                            ParserState::InAref(a)     => cell.arefs.push(ArefInstance    { cell_name: a.cell_name, origin: a.origin, rotation_deg: a.rotation_deg, magnification: a.magnification, mirror_x: a.mirror_x, cols: a.cols, rows: a.rows, col_vector: a.col_vector, row_vector: a.row_vector }),
                            _ => {}
                        }
                    }
                }

                _ => {} // safely ignore PROPATTR, PROPVALUE, ELFLAGS, PLEX, etc.
            }
        }

        layout.find_top_cell();
        Ok(layout)
    }
}

impl GdsParser<std::io::BufReader<std::fs::File>> {
    pub fn from_path(path: impl AsRef<std::path::Path>) -> Result<Self, GdsError> {
        Ok(GdsParser::new(GdsReader::from_path(path)?))
    }
}

//Tests 

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_parse_complex_gds() {
        let path = "../../../data/samples/complex.gds";
        if std::path::Path::new(path).exists() {
            let bytes = std::fs::read(path).unwrap();
            let reader = super::GdsReader::new(std::io::Cursor::new(bytes));
            let mut parser = super::GdsParser::new(reader);
            match parser.parse() {
                Ok(layout) => println!("Success! Cells: {}", layout.cells.len()),
                Err(e) => panic!("Error parsing complex.gds: {:?}", e),
            }
        }
    }

    fn rec(rt: u8, dt: u8, data: &[u8]) -> Vec<u8> {
        let len = (4 + data.len()) as u16;
        let mut b = len.to_be_bytes().to_vec();
        b.push(rt); b.push(dt); b.extend_from_slice(data); b
    }
    fn i16_rec(rt: u8, v: i16)     -> Vec<u8> { rec(rt, 0x02, &v.to_be_bytes()) }
    #[allow(dead_code)]
    fn i32_rec(rt: u8, v: i32)     -> Vec<u8> { rec(rt, 0x03, &v.to_be_bytes()) }
    fn str_rec(rt: u8, s: &str)    -> Vec<u8> {
        let mut d = s.as_bytes().to_vec();
        if d.len() % 2 != 0 { d.push(0); }
        rec(rt, 0x06, &d)
    }
    fn xy_rec(pts: &[(i32, i32)])  -> Vec<u8> {
        let mut d = Vec::new();
        for (x, y) in pts { d.extend_from_slice(&x.to_be_bytes()); d.extend_from_slice(&y.to_be_bytes()); }
        rec(0x10, 0x03, &d)
    }
    fn nodata(rt: u8) -> Vec<u8> { rec(rt, 0x00, &[]) }
    fn timestamps()   -> Vec<u8> { vec![0u8; 24] }

    fn minimal_gds_with_polygon() -> Vec<u8> {
        let mut b: Vec<u8> = Vec::new();
        b.extend(rec(0x00, 0x02, &0x03i16.to_be_bytes()));     // HEADER v3
        b.extend(rec(0x01, 0x02, &timestamps()));              // BGNLIB
        b.extend(str_rec(0x02, "TESTLIB"));                    // LIBNAME
        b.extend(rec(0x03, 0x05, &[0u8; 16]));                // UNITS (zeroed)
        b.extend(rec(0x05, 0x02, &timestamps()));              // BGNSTR
        b.extend(str_rec(0x06, "TOP"));                        // STRNAME "TOP"
        b.extend(nodata(0x08));                                // BOUNDARY
        b.extend(i16_rec(0x0D, 1));                            // LAYER 1
        b.extend(i16_rec(0x0E, 0));                            // DATATYPE 0
        b.extend(xy_rec(&[(0,0),(10,0),(10,10),(0,10),(0,0)]));// XY 5 pts
        b.extend(nodata(0x11));                                // ENDEL
        b.extend(nodata(0x07));                                // ENDSTR
        b.extend(nodata(0x04));                                // ENDLIB
        b
    }

    fn gds_with_hierarchy() -> Vec<u8> {
        let mut b: Vec<u8> = Vec::new();
        b.extend(rec(0x00, 0x02, &0x03i16.to_be_bytes()));
        b.extend(rec(0x01, 0x02, &timestamps()));
        b.extend(str_rec(0x02, "HIER"));
        b.extend(rec(0x03, 0x05, &[0u8; 16]));

        // UNIT cell: one polygon
        b.extend(rec(0x05, 0x02, &timestamps()));
        b.extend(str_rec(0x06, "UNIT"));
        b.extend(nodata(0x08));
        b.extend(i16_rec(0x0D, 2));
        b.extend(i16_rec(0x0E, 0));
        b.extend(xy_rec(&[(0,0),(5,0),(5,5),(0,5),(0,0)]));
        b.extend(nodata(0x11));
        b.extend(nodata(0x07));

        // TOP cell: sref to UNIT
        b.extend(rec(0x05, 0x02, &timestamps()));
        b.extend(str_rec(0x06, "TOP"));
        b.extend(nodata(0x0A));                                // SREF
        b.extend(str_rec(0x12, "UNIT"));                      // SNAME
        b.extend(xy_rec(&[(100, 200)]));                      // XY placement
        b.extend(nodata(0x11));                                // ENDEL
        b.extend(nodata(0x07));

        b.extend(nodata(0x04));
        b
    }

    #[test]
    fn parses_library_name() {
        let bytes = minimal_gds_with_polygon();
        let layout = GdsParser::new(GdsReader::new(Cursor::new(bytes))).parse().unwrap();
        assert_eq!(layout.lib_name, "TESTLIB");
    }

    #[test]
    fn parses_cell_name() {
        let bytes = minimal_gds_with_polygon();
        let layout = GdsParser::new(GdsReader::new(Cursor::new(bytes))).parse().unwrap();
        assert!(layout.cells.contains_key("TOP"));
    }

    #[test]
    fn parses_polygon_shape() {
        let bytes = minimal_gds_with_polygon();
        let layout = GdsParser::new(GdsReader::new(Cursor::new(bytes))).parse().unwrap();
        let cell = &layout.cells["TOP"];
        assert_eq!(cell.shapes.len(), 1);
        if let Shape::Polygon { layer, datatype, points } = &cell.shapes[0] {
            assert_eq!(*layer, 1);
            assert_eq!(*datatype, 0);
            assert_eq!(points.len(), 5);
            assert_eq!(points[0], (0.0, 0.0));
            assert_eq!(points[1], (10.0, 0.0));
        } else {
            panic!("expected Polygon");
        }
    }

    #[test]
    fn parses_sref_instance() {
        let bytes = gds_with_hierarchy();
        let layout = GdsParser::new(GdsReader::new(Cursor::new(bytes))).parse().unwrap();
        let top = &layout.cells["TOP"];
        assert_eq!(top.srefs.len(), 1);
        let sref = &top.srefs[0];
        assert_eq!(sref.cell_name, "UNIT");
        assert_eq!(sref.position, (100.0, 200.0));
        assert_eq!(sref.magnification, 1.0);
        assert!(!sref.mirror_x);
    }

    #[test]
    fn find_top_cell_resolves_root() {
        let bytes = gds_with_hierarchy();
        let layout = GdsParser::new(GdsReader::new(Cursor::new(bytes))).parse().unwrap();
        assert_eq!(layout.top_cell.as_deref(), Some("TOP"));
    }

    #[test]
    fn layer_auto_registered_on_parse() {
        let bytes = minimal_gds_with_polygon();
        let layout = GdsParser::new(GdsReader::new(Cursor::new(bytes))).parse().unwrap();
        assert!(layout.layers.contains_key(&1));
    }

    #[test]
    fn returns_error_on_truncated_stream() {
        let bytes = vec![0x00, 0x06, 0x00, 0x02]; // HEADER header only, then EOF
        let result = GdsParser::new(GdsReader::new(Cursor::new(bytes))).parse();
        assert!(result.is_err());
    }

    #[test]
    fn ibm_real8_decodes_one() {
        // IBM float encoding of 1.0: sign=0, exp=65 (0x41), mantissa=0x10_0000_0000_0000
        let bytes = [0x41u8, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let v = decode_ibm_real8(&bytes);
        assert!((v - 1.0).abs() < 1e-12, "expected 1.0, got {v}");
    }

    #[test]
    fn ibm_real8_decodes_zero_bytes_as_zero() {
        let bytes = [0u8; 8];
        assert_eq!(decode_ibm_real8(&bytes), 0.0);
    }
}

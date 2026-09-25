use std::fs::File;
use std::io::BufReader;
use std::path::Path;

fn main() {
    let path = Path::new(r"C:\Users\tuelo\OneDrive\Documents\Rust\caravel.gds");
    let reader = gds_parser::reader::GdsReader::new(BufReader::new(File::open(path).unwrap()));
    let parser = gds_parser::parser::GdsParser::new(reader);
    let layout = parser.parse().unwrap();
    
    let wrapper = layout.cells.get("user_proj_example").unwrap();
    println!("user_proj_example has:");
    println!("  {} shapes", wrapper.shapes.len());
    println!("  {} SREFs", wrapper.srefs.len());
    println!("  {} AREFs", wrapper.arefs.len());
    
    // Count layers of shapes
    let mut layers = std::collections::HashMap::new();
    for shape in &wrapper.shapes {
        use layout_db::cell::Shape;
        let layer = match shape {
            Shape::Polygon { layer, .. } => *layer,
            Shape::Path { layer, .. } => *layer,
            Shape::Text { layer, .. } => *layer,
        };
        *layers.entry(layer).or_insert(0) += 1;
        if layer == 235 {
            let bb = shape.bounding_box();
            println!("Layer 235 shape BB: {:?}", bb);
        }
    }
    
    println!("Top-level shapes by layer:");
    for (layer, count) in layers {
        println!("  Layer {}: {}", layer, count);
    }
}

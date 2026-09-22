import gdstk
import os
import math

def generate_complex():
    lib = gdstk.Library()

    # 1. Base Unit Cell with complex geometry and many layers
    unit_cell = lib.new_cell('UNIT')
    
    # Generate 100 layers, each with a different datatype
    for layer in range(1, 101):
        datatype = layer % 10
        
        # High vertex count polygon (approximating a circle with 512 points)
        circle_poly = gdstk.ellipse((0, 0), (50, 50), tolerance=0.1, layer=layer, datatype=datatype)
        
        # A complex path (a spiral)
        path = gdstk.FlexPath((0, 0), 2, layer=layer, datatype=datatype)
        pts = []
        for i in range(1, 50):
            r = i * 2
            theta = i * 0.5
            pts.append((r * math.cos(theta), r * math.sin(theta)))
        path.interpolation(pts)
            
        unit_cell.add(circle_poly, path)

    # 2. Deep Nesting (20 levels deep)
    current_cell = unit_cell
    for depth in range(1, 21):
        next_cell = lib.new_cell(f'NEST_{depth}')
        
        # Apply rotation, magnification, and mirroring at each level to stress transform logic
        ref = gdstk.Reference(
            current_cell, 
            origin=(10, 10), 
            rotation=math.pi / 8, # 22.5 degrees
            magnification=0.95, 
            x_reflection=(depth % 2 == 0)
        )
        next_cell.add(ref)
        current_cell = next_cell

    # 3. Array References (AREF)
    block_cell = lib.new_cell('BLOCK')
    # 20x20 Array of the deeply nested cell
    aref = gdstk.Reference(
        current_cell, 
        origin=(0, 0), 
        columns=20, 
        rows=20, 
        spacing=(500, 500)
    )
    block_cell.add(aref)

    # 4. Top Cell with more arrays and scattered instances
    top_cell = lib.new_cell('TOP')
    # 5x5 blocks
    top_aref = gdstk.Reference(
        block_cell,
        origin=(0, 0),
        columns=5,
        rows=5,
        spacing=(12000, 12000)
    )
    top_cell.add(top_aref)
    
    # Add some massive background polygons
    bg_poly = gdstk.rectangle((-5000, -5000), (60000, 60000), layer=200, datatype=0)
    top_cell.add(bg_poly)

    # Save to file
    out_dir = os.path.dirname(os.path.abspath(__file__))
    out_path = os.path.join(out_dir, 'complex.gds')
    
    lib.write_gds(out_path)
    print(f"Generated {out_path}")
    print(f"  - Layers: 100")
    print(f"  - Nesting Depth: 20")
    print(f"  - Top Cell AREF: 5x5 Blocks -> 20x20 Nested instances")
    print(f"  - Total Instances: {5*5*20*20} deep trees")

if __name__ == '__main__':
    generate_complex()

import gdstk
import os

def generate():
    # Create a library
    lib = gdstk.Library()

    # 1. UNIT cell: 3 Polygons on layers 1 and 2
    unit_cell = lib.new_cell('UNIT')
    
    # Layer 1 shapes
    poly1 = gdstk.rectangle((0, 0), (10, 10), layer=1)
    poly2 = gdstk.rectangle((15, 0), (25, 10), layer=1)
    # Layer 2 shape
    poly3 = gdstk.rectangle((0, 15), (25, 25), layer=2)
    
    unit_cell.add(poly1, poly2, poly3)

    # 2. BLOCK cell: 4 SREFs to UNIT cell
    block_cell = lib.new_cell('BLOCK')
    for i in range(2):
        for j in range(2):
            # Place 2x2 grid of UNIT cells, spaced by 30 units
            ref = gdstk.Reference(unit_cell, (i * 30, j * 30))
            block_cell.add(ref)

    # 3. TOP cell: 5 SREFs to BLOCK cell in a row
    top_cell = lib.new_cell('TOP')
    for i in range(5):
        # Place 5 BLOCK cells, spaced by 80 units
        ref = gdstk.Reference(block_cell, (i * 80, 0))
        top_cell.add(ref)

    # Save to disk
    os.makedirs(os.path.dirname(__file__), exist_ok=True)
    out_path = os.path.join(os.path.dirname(__file__), 'simple.gds')
    lib.write_gds(out_path)
    print(f"Generated {out_path} with a 3-level hierarchy.")
    print("Total flat shapes: 5 (blocks) * 4 (units) * 3 (polygons) = 60")

if __name__ == '__main__':
    generate()

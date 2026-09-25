import { Application, Graphics, GraphicsContext, Container, Matrix } from 'pixi.js';
import { IShape, IPolygon, IPath, ILayerInfo, IViewportResponse, ICellDefinition, IInstance, isPolygon, isPath, isText } from '../types';
import { Viewport } from './viewport';

class CellFactory {
  public def: ICellDefinition;
  private renderer: LayoutRenderer;

  constructor(def: ICellDefinition, renderer: LayoutRenderer) {
    this.def = def;
    this.renderer = renderer;
  }

  public draw(ctx: GraphicsContext, parentMatrix: Matrix, zoom: number, tileBBox: [number, number, number, number]): void {
    if (this.def.bbox && this.def.bbox.length === 4) {
      const [minX, minY, maxX, maxY] = this.def.bbox;
      const pts = [
        [minX, minY], [maxX, minY], [maxX, maxY], [minX, maxY]
      ];
      const tPts = pts.map(p => this.transformPoint(p[0]!, p[1]!, parentMatrix));
      const wMinX = Math.min(...tPts.map(p => p[0]!));
      const wMaxX = Math.max(...tPts.map(p => p[0]!));
      const wMinY = Math.min(...tPts.map(p => p[1]!));
      const wMaxY = Math.max(...tPts.map(p => p[1]!));
      
      // Spatial Culling: Skip if this cell doesn't intersect the tile
      if (wMaxX < tileBBox[0] || wMinX > tileBBox[2] || wMaxY < tileBBox[1] || wMinY > tileBBox[3]) {
        return;
      }
      
      // Screen Size Culling: Draw abstract box and stop if too small
      const widthScreen = (wMaxX - wMinX) * zoom;
      const heightScreen = (wMaxY - wMinY) * zoom;
      
      if (widthScreen < 3.0 && heightScreen < 3.0) {
        ctx.moveTo(tPts[0]![0], tPts[0]![1]);
        for (let i = 1; i < tPts.length; i++) ctx.lineTo(tPts[i]![0], tPts[i]![1]);
        ctx.lineTo(tPts[0]![0], tPts[0]![1]);
        ctx.fill({ color: 0x666666, alpha: 0.4 });
        return;
      }
    }

    if (this.def.shapes.length > 0) {
      this.drawShapesToContext(ctx, this.def.shapes, parentMatrix, tileBBox, zoom);
    } else if (this.def.instances.length === 0 && this.def.bbox && this.def.bbox.length === 4) {
      // Abstract cell fallback
      const [minX, minY, maxX, maxY] = this.def.bbox;
      if (maxX > minX && maxY > minY) {
        const pts = [
          [minX, minY], [maxX, minY], [maxX, maxY], [minX, maxY]
        ];
        const tPts = pts.map(p => this.transformPoint(p[0]!, p[1]!, parentMatrix));
        ctx.moveTo(tPts[0]![0], tPts[0]![1]);
        for (let i = 1; i < tPts.length; i++) ctx.lineTo(tPts[i]![0], tPts[i]![1]);
        ctx.lineTo(tPts[0]![0], tPts[0]![1]);
        ctx.fill({ color: 0x555555, alpha: 0.2 });
        ctx.stroke({ width: (maxX - minX) * 0.001 * parentMatrix.a, color: 0x888888, alpha: 0.8 });
      }
    }
    
    for (const inst of this.def.instances) {
      const factory = this.renderer.getCellFactory(inst.cellName);
      if (factory) {
        const cols = inst.cols || 1;
        const rows = inst.rows || 1;
        const totalInstances = cols * rows;
        
        let drawAsBBox = false;
        if (totalInstances >= 100) {
          const bbox = factory.def.bbox;
          if (bbox && bbox.length === 4) {
            const m = inst.transform;
            const baseMatrix = new Matrix(m[0], m[3], m[1], m[4], m[2], m[5]);
            const instMatrix = parentMatrix.clone().append(baseMatrix);
            const scale = Math.sqrt(instMatrix.a * instMatrix.a + instMatrix.b * instMatrix.b);
            const instWidthScreen = (bbox[2] - bbox[0]) * scale * zoom;
            const instHeightScreen = (bbox[3] - bbox[1]) * scale * zoom;
            
            if (instWidthScreen < 5.0 && instHeightScreen < 5.0) {
              drawAsBBox = true;
            }
          }
        }
        
        if (drawAsBBox) {
          const bbox = factory.def.bbox!;
          const [minX, minY, maxX, maxY] = bbox;
          const colVec = inst.colVector || [0, 0];
          const rowVec = inst.rowVector || [0, 0];
          
          const spanX = (cols - 1) * colVec[0] + (rows - 1) * rowVec[0];
          const spanY = (cols - 1) * colVec[1] + (rows - 1) * rowVec[1];
          
          const xs = [minX, maxX, minX + spanX, maxX + spanX];
          const ys = [minY, maxY, minY + spanY, maxY + spanY];
          const arrayMinX = Math.min(...xs);
          const arrayMaxX = Math.max(...xs);
          const arrayMinY = Math.min(...ys);
          const arrayMaxY = Math.max(...ys);
          
          const m = inst.transform;
          const localMatrix = new Matrix(m[0], m[3], m[1], m[4], m[2], m[5]);
          const instMatrix = parentMatrix.clone().append(localMatrix);
          
          const pts = [
            [arrayMinX, arrayMinY], [arrayMaxX, arrayMinY], [arrayMaxX, arrayMaxY], [arrayMinX, arrayMaxY]
          ];
          const tPts = pts.map(p => this.transformPoint(p[0]!, p[1]!, instMatrix));
          
          ctx.moveTo(tPts[0]![0], tPts[0]![1]);
          for (let i = 1; i < tPts.length; i++) ctx.lineTo(tPts[i]![0], tPts[i]![1]);
          ctx.lineTo(tPts[0]![0], tPts[0]![1]);
          
          ctx.fill({ color: 0x555555, alpha: 0.5 });
          ctx.stroke({ width: 0.1 * instMatrix.a, color: 0x888888, alpha: 0.8 });
        } else {
          const m = inst.transform;
          const baseMatrix = new Matrix(m[0], m[3], m[1], m[4], m[2], m[5]);
          const colVec = inst.colVector || [0, 0];
          const rowVec = inst.rowVector || [0, 0];
          
          let startI = 0, endI = cols - 1;
          let startJ = 0, endJ = rows - 1;

          if (factory.def.bbox && (cols > 1 || rows > 1)) {
            const [bMinX, bMinY, bMaxX, bMaxY] = factory.def.bbox;
            const instMatrix = parentMatrix.clone().append(baseMatrix);
            const invMatrix = instMatrix.clone().invert();
            
            const tCorners = [
              [tileBBox[0], tileBBox[1]], [tileBBox[2], tileBBox[1]], 
              [tileBBox[2], tileBBox[3]], [tileBBox[0], tileBBox[3]]
            ].map(p => this.transformPoint(p[0]!, p[1]!, invMatrix));
            
            const invTileMinX = Math.min(...tCorners.map(p => p[0]!));
            const invTileMaxX = Math.max(...tCorners.map(p => p[0]!));
            const invTileMinY = Math.min(...tCorners.map(p => p[1]!));
            const invTileMaxY = Math.max(...tCorners.map(p => p[1]!));
            
            const targetMinX = invTileMinX - bMaxX;
            const targetMaxX = invTileMaxX - bMinX;
            const targetMinY = invTileMinY - bMaxY;
            const targetMaxY = invTileMaxY - bMinY;
            
            const cx = colVec[0]!, cy = colVec[1]!;
            const rx = rowVec[0]!, ry = rowVec[1]!;
            const det = cx * ry - rx * cy;
            
            if (Math.abs(det) > 1e-6) {
              const mapIJ = (x: number, y: number) => [
                (x * ry - y * rx) / det,
                (cx * y - cy * x) / det
              ];
              
              const ijCorners = [
                mapIJ(targetMinX, targetMinY),
                mapIJ(targetMaxX, targetMinY),
                mapIJ(targetMaxX, targetMaxY),
                mapIJ(targetMinX, targetMaxY)
              ];
              
              const minI = Math.floor(Math.min(...ijCorners.map(p => p[0]!)));
              const maxI = Math.ceil(Math.max(...ijCorners.map(p => p[0]!)));
              const minJ = Math.floor(Math.min(...ijCorners.map(p => p[1]!)));
              const maxJ = Math.ceil(Math.max(...ijCorners.map(p => p[1]!)));
              
              startI = Math.max(0, minI);
              endI = Math.min(cols - 1, maxI);
              startJ = Math.max(0, minJ);
              endJ = Math.min(rows - 1, maxJ);
            } else if (cx !== 0 || cy !== 0) {
                const len2 = cx*cx + cy*cy;
                const mapI = (x: number, y: number) => (x*cx + y*cy) / len2;
                const iCorners = [
                  mapI(targetMinX, targetMinY),
                  mapI(targetMaxX, targetMinY),
                  mapI(targetMaxX, targetMaxY),
                  mapI(targetMinX, targetMaxY)
                ];
                const minI = Math.floor(Math.min(...iCorners));
                const maxI = Math.ceil(Math.max(...iCorners));
                startI = Math.max(0, minI);
                endI = Math.min(cols - 1, maxI);
            } else if (rx !== 0 || ry !== 0) {
                const len2 = rx*rx + ry*ry;
                const mapJ = (x: number, y: number) => (x*rx + y*ry) / len2;
                const jCorners = [
                  mapJ(targetMinX, targetMinY),
                  mapJ(targetMaxX, targetMinY),
                  mapJ(targetMaxX, targetMaxY),
                  mapJ(targetMinX, targetMaxY)
                ];
                const minJ = Math.floor(Math.min(...jCorners));
                const maxJ = Math.ceil(Math.max(...jCorners));
                startJ = Math.max(0, minJ);
                endJ = Math.min(rows - 1, maxJ);
            }
          }
          
          if (startI <= endI && startJ <= endJ) {
            for (let i = startI; i <= endI; i++) {
              for (let j = startJ; j <= endJ; j++) {
                const dx = i * colVec[0]! + j * rowVec[0]!;
                const dy = i * colVec[1]! + j * rowVec[1]!;
                
                const localMatrix = baseMatrix.clone().translate(dx, dy);
                const instMatrix = parentMatrix.clone().append(localMatrix);
                
                factory.draw(ctx, instMatrix, zoom, tileBBox);
              }
            }
          }
        }
      }
    }
  }

  private transformPoint(x: number, y: number, matrix: Matrix): [number, number] {
    return [
      matrix.a * x + matrix.c * y + matrix.tx,
      matrix.b * x + matrix.d * y + matrix.ty
    ];
  }

  private drawShapesToContext(ctx: GraphicsContext, shapes: IShape[], matrix: Matrix, tileBBox: [number, number, number, number], zoom: number) {
    for (const shape of shapes) {
      if (isText(shape)) continue;
      
      const layerInfo = this.renderer.layers.get(shape.layer);
      const colorStr = layerInfo?.color || '#999999';
      const colorNum = parseInt(colorStr.replace('#', '0x'), 16);

      if (isPolygon(shape)) {
        const pts = shape.points;
        if (pts.length < 3) continue;

        let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
        for (let i = 0; i < pts.length; i++) {
          const p = pts[i]!;
          const tX = matrix.a * p[0] + matrix.c * p[1] + matrix.tx;
          const tY = matrix.b * p[0] + matrix.d * p[1] + matrix.ty;
          if (tX < minX) minX = tX;
          if (tX > maxX) maxX = tX;
          if (tY < minY) minY = tY;
          if (tY > maxY) maxY = tY;
        }
        
        if (maxX < tileBBox[0] || minX > tileBBox[2] || maxY < tileBBox[1] || minY > tileBBox[3]) {
          continue;
        }
        
        const widthScreen = (maxX - minX) * zoom;
        const heightScreen = (maxY - minY) * zoom;
        if (widthScreen < 1.0 && heightScreen < 1.0) {
          continue;
        }
        
        const p0 = pts[0]!;
        const firstX = matrix.a * p0[0] + matrix.c * p0[1] + matrix.tx;
        const firstY = matrix.b * p0[0] + matrix.d * p0[1] + matrix.ty;
        ctx.moveTo(firstX, firstY);
        for (let i = 1; i < pts.length; i++) {
          const p = pts[i]!;
          const tX = matrix.a * p[0] + matrix.c * p[1] + matrix.tx;
          const tY = matrix.b * p[0] + matrix.d * p[1] + matrix.ty;
          ctx.lineTo(tX, tY);
        }
        ctx.lineTo(firstX, firstY);
        
        ctx.fill({ color: colorNum, alpha: 0.1 });
        // Use a minimum stroke width of 0.5 screen pixels, scaled back to world coordinates
        const screenToWorld = 1.0 / zoom;
        const strokeW = Math.max(0.1, 0.5 * screenToWorld) / matrix.a;
        ctx.stroke({ width: strokeW, color: colorNum, alpha: 0.8 });
        
      } else if (isPath(shape)) {
        const pts = shape.points;
        if (pts.length === 0) continue;
        
        let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
        for (let i = 0; i < pts.length; i++) {
          const p = pts[i]!;
          const tX = matrix.a * p[0] + matrix.c * p[1] + matrix.tx;
          const tY = matrix.b * p[0] + matrix.d * p[1] + matrix.ty;
          if (tX < minX) minX = tX;
          if (tX > maxX) maxX = tX;
          if (tY < minY) minY = tY;
          if (tY > maxY) maxY = tY;
        }
        
        const halfW = (shape.width / 2) * matrix.a;
        if (maxX + halfW < tileBBox[0] || minX - halfW > tileBBox[2] || maxY + halfW < tileBBox[1] || minY - halfW > tileBBox[3]) {
          continue;
        }
        
        const widthScreen = (maxX - minX + shape.width * matrix.a) * zoom;
        const heightScreen = (maxY - minY + shape.width * matrix.a) * zoom;
        if (widthScreen < 1.0 && heightScreen < 1.0) {
          continue;
        }
        
        const p0 = pts[0]!;
        const firstX = matrix.a * p0[0] + matrix.c * p0[1] + matrix.tx;
        const firstY = matrix.b * p0[0] + matrix.d * p0[1] + matrix.ty;
        ctx.moveTo(firstX, firstY);
        for (let i = 1; i < pts.length; i++) {
          const p = pts[i]!;
          const tX = matrix.a * p[0] + matrix.c * p[1] + matrix.tx;
          const tY = matrix.b * p[0] + matrix.d * p[1] + matrix.ty;
          ctx.lineTo(tX, tY);
        }
        
        const cap = shape.pathType === 0 ? 'square' : shape.pathType === 1 ? 'round' : 'butt';
        ctx.stroke({ width: shape.width * matrix.a, color: colorNum, alpha: 0.7, cap, join: 'round' });
      }
    }
  }
}

export class LayoutRenderer {
  private canvas: HTMLCanvasElement;
  private app: Application;
  private viewport: Viewport;
  public layers: Map<number, ILayerInfo> = new Map();
  
  public cellFactories: Map<string, CellFactory> = new Map();
  public knownCells: Map<string, "cell" | "block" | "shape"> = new Map();
  
  private mainContainer: Container;
  private graphicsContext: GraphicsContext;

  constructor(canvas: HTMLCanvasElement, viewport: Viewport) {
    this.canvas = canvas;
    this.viewport = viewport;
    this.app = new Application();
    this.mainContainer = new Container();
    this.graphicsContext = new GraphicsContext();
  }

  async init() {
    await this.app.init({
      canvas: this.canvas,
      width: this.canvas.width,
      height: this.canvas.height,
      backgroundAlpha: 0,
      antialias: true,
      resolution: window.devicePixelRatio || 1,
      autoDensity: true,
    });
    
    const mainGraphics = new Graphics(this.graphicsContext);
    this.mainContainer.addChild(mainGraphics);
    this.app.stage.addChild(this.mainContainer);
  }

  resize(width: number, height: number) {
    if (this.app.renderer) {
      this.app.renderer.resize(width, height);
    }
  }
  
  getCellFactory(name: string): CellFactory | undefined {
    return this.cellFactories.get(name);
  }

  setViewportData(res: IViewportResponse) {
    const lodScores = { 'cell': 0, 'block': 1, 'shape': 2 };

    // Process new cell definitions
    if (res.cellDefinitions) {
      for (const [name, def] of Object.entries(res.cellDefinitions)) {
        if (!this.cellFactories.has(name)) {
          this.cellFactories.set(name, new CellFactory(def, this));
          this.knownCells.set(name, res.lodLevel);
        } else {
          const existingLod = this.knownCells.get(name) || 'cell';
          if (lodScores[res.lodLevel] > lodScores[existingLod]) {
            this.cellFactories.set(name, new CellFactory(def, this));
            this.knownCells.set(name, res.lodLevel);
          }
        }
      }
    }
    
    // Update top level LOD tracking
    for (const inst of res.instances) {
      const current = this.knownCells.get(inst.cellName);
      if (!current || lodScores[res.lodLevel] > lodScores[current]) {
        this.knownCells.set(inst.cellName, res.lodLevel);
      }
    }

    this.graphicsContext.clear();
    const zoom = this.viewport.zoom;
    
    const bbox = this.viewport.getBoundingBox();
    const viewportBBox: [number, number, number, number] = [bbox.x1, bbox.y1, bbox.x2, bbox.y2];

    // Render top-level flat shapes
    if (res.shapes.length > 0) {
      const topLevelDef: ICellDefinition = { shapes: res.shapes, instances: [], bbox: viewportBBox };
      const topLevelFactory = new CellFactory(topLevelDef, this);
      topLevelFactory.draw(this.graphicsContext, Matrix.IDENTITY, zoom, viewportBBox);
    }

    // Process all top-level instances directly into the GraphicsContext
    for (const inst of res.instances) {
      const factory = this.cellFactories.get(inst.cellName);
      if (!factory) continue;

      const cols = inst.cols || 1;
      const rows = inst.rows || 1;
      
      const m = inst.transform;
      const baseMatrix = new Matrix(m[0], m[3], m[1], m[4], m[2], m[5]);
      const colVec = inst.colVector || [0, 0];
      const rowVec = inst.rowVector || [0, 0];
      
      for (let i = 0; i < cols; i++) {
        for (let j = 0; j < rows; j++) {
          const dx = i * colVec[0] + j * rowVec[0];
          const dy = i * colVec[1] + j * rowVec[1];
          const localMatrix = baseMatrix.clone().translate(dx, dy);
          factory.draw(this.graphicsContext, localMatrix, zoom, viewportBBox);
        }
      }
    }
  }

  clearData() {
    this.graphicsContext.clear();
    this.cellFactories.clear();
    this.knownCells.clear();
  }

  render() {
    if (!this.viewport) return;
    this.app.stage.scale.set(this.viewport.zoom, -this.viewport.zoom);
    this.app.stage.position.set(this.viewport.panX, this.viewport.panY);
  }
}

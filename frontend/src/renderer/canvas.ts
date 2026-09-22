import { IShape, IPolygon, IPath, ILayerInfo, isPolygon, isPath, isText } from '../types';
import { Viewport } from './viewport';

export class LayoutRenderer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private viewport: Viewport;
  public layers: Map<number, ILayerInfo> = new Map();

  constructor(canvas: HTMLCanvasElement, viewport: Viewport) {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d')!;
    this.viewport = viewport;
  }

  render(shapes: IShape[]) {
    this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);

    for (const shape of shapes) {
      if (isText(shape)) continue;
      
      const color = this.layers.get(shape.layer)?.color || '#999999';

      if (isPolygon(shape)) {
        this.drawPolygon(shape, color);
      } else if (isPath(shape)) {
        this.drawPath(shape, color);
      }
    }
  }

  private transform(x: number, y: number): [number, number] {
    return [
      this.viewport.panX + x * this.viewport.zoom,
      this.viewport.panY - y * this.viewport.zoom
    ];
  }

  private drawPolygon(shape: IPolygon, color: string) {
    if (shape.points.length === 0) return;

    this.ctx.fillStyle = color;
    this.ctx.beginPath();
    const [startX, startY] = this.transform(shape.points[0]![0], shape.points[0]![1]);
    this.ctx.moveTo(startX, startY);

    for (let i = 1; i < shape.points.length; i++) {
      const [x, y] = this.transform(shape.points[i]![0], shape.points[i]![1]);
      this.ctx.lineTo(x, y);
    }

    this.ctx.globalAlpha = 0.7;
    this.ctx.fill();
    this.ctx.globalAlpha = 1.0;
    
    this.ctx.strokeStyle = color;
    this.ctx.lineWidth = 1;
    this.ctx.stroke();
  }

  private drawPath(shape: IPath, color: string) {
    if (shape.points.length === 0) return;

    this.ctx.strokeStyle = color;
    // Scale width
    this.ctx.lineWidth = Math.max(1, shape.width * this.viewport.zoom);
    this.ctx.lineCap = shape.pathType === 0 ? 'square' : shape.pathType === 1 ? 'round' : 'butt';
    this.ctx.lineJoin = 'round';

    this.ctx.beginPath();
    const [startX, startY] = this.transform(shape.points[0]![0], shape.points[0]![1]);
    this.ctx.moveTo(startX, startY);

    for (let i = 1; i < shape.points.length; i++) {
      const [x, y] = this.transform(shape.points[i]![0], shape.points[i]![1]);
      this.ctx.lineTo(x, y);
    }
    
    this.ctx.globalAlpha = 0.7;
    this.ctx.stroke();
    this.ctx.globalAlpha = 1.0;
  }
}

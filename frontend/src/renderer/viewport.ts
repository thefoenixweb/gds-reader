import { IViewportQuery } from '../types';

export class Viewport {
  width: number;
  height: number;
  panX: number;
  panY: number;
  zoom: number;

  constructor(width: number, height: number) {
    this.width = width;
    this.height = height;
    // Default: center world origin on screen
    this.panX = width / 2;
    this.panY = height / 2;
    this.zoom = 1.0;
  }

  /**
   * Converts world coordinates (GDS Y-up) to screen coordinates (Canvas Y-down).
   */
  worldToScreen(wx: number, wy: number): [number, number] {
    return [
      this.panX + wx * this.zoom,
      this.panY - wy * this.zoom,
    ];
  }

  /**
   * Converts screen coordinates (Canvas Y-down) to world coordinates (GDS Y-up).
   */
  screenToWorld(sx: number, sy: number): [number, number] {
    return [
      (sx - this.panX) / this.zoom,
      (this.panY - sy) / this.zoom,
    ];
  }

  /**
   * Returns a bounding box covering the currently visible screen area.
   */
  getBoundingBox(): IViewportQuery {
    // Bottom-left of screen is (0, height)
    const [wx1, wy1] = this.screenToWorld(0, this.height);
    // Top-right of screen is (width, 0)
    const [wx2, wy2] = this.screenToWorld(this.width, 0);

    return {
      x1: wx1,
      y1: wy1,
      x2: wx2,
      y2: wy2,
      zoom: this.zoom,
      visibleLayers: [] // To be filled by caller
    };
  }

  /**
   * Updates zoom while keeping the world point under the mouse stationary.
   */
  handleWheel(e: WheelEvent) {
    // Standardize delta
    const zoomFactor = 1.1;
    const delta = e.deltaY > 0 ? (1 / zoomFactor) : zoomFactor;

    // Get world coordinate under mouse
    const [wx, wy] = this.screenToWorld(e.offsetX, e.offsetY);
    
    // Update zoom
    this.zoom *= delta;
    
    // Adjust pan so the same world coordinate remains under the mouse
    this.panX = e.offsetX - wx * this.zoom;
    this.panY = e.offsetY + wy * this.zoom;
  }

  /**
   * Updates pan by screen delta.
   */
  handleMouseDrag(dx: number, dy: number) {
    this.panX += dx;
    this.panY += dy;
  }
}

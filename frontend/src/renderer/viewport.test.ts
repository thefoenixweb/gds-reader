import { describe, it, expect } from 'vitest';
import { Viewport } from './viewport';

describe('Viewport', () => {
  it('centers world origin on initialization', () => {
    const vp = new Viewport(800, 600);
    const [sx, sy] = vp.worldToScreen(0, 0);
    expect(sx).toBe(400);
    expect(sy).toBe(300);
  });

  it('worldToScreen transforms coordinates correctly', () => {
    const vp = new Viewport(800, 600);
    vp.zoom = 2.0;
    vp.panX = 100;
    vp.panY = 50;

    const [sx, sy] = vp.worldToScreen(100, 50);
    expect(sx).toBeCloseTo(300); // 100 + 100*2 = 300
    expect(sy).toBeCloseTo(-50); // 50 - 50*2 = -50
  });

  it('screenToWorld transforms coordinates correctly', () => {
    const vp = new Viewport(800, 600);
    vp.zoom = 2.0;
    vp.panX = 100;
    vp.panY = 50;

    const [wx, wy] = vp.screenToWorld(300, -50);
    expect(wx).toBeCloseTo(100);
    expect(wy).toBeCloseTo(50);
  });

  it('getBoundingBox returns correct world coordinates covering the screen', () => {
    const vp = new Viewport(800, 600);
    // Defaults: panX=400, panY=300, zoom=1
    const box = vp.getBoundingBox();

    // Bottom-left screen (0, 600): wx = (0-400)/1 = -400, wy = (300-600)/1 = -300
    expect(box.x1).toBe(-400);
    expect(box.y1).toBe(-300);

    // Top-right screen (800, 0): wx = (800-400)/1 = 400, wy = (300-0)/1 = 300
    expect(box.x2).toBe(400);
    expect(box.y2).toBe(300);
  });

  it('handleMouseDrag updates pan', () => {
    const vp = new Viewport(800, 600);
    vp.handleMouseDrag(10, -20);
    expect(vp.panX).toBe(410);
    expect(vp.panY).toBe(280);
  });
});

/**
 * @vitest-environment jsdom
 */
import { describe, it, expect, vi } from 'vitest';
import { LayoutRenderer } from './canvas';
import { Viewport } from './viewport';
import { IPolygon } from '../types';

describe('LayoutRenderer', () => {
  it('renders a polygon and calls ctx.fill once', () => {
    // 1. Setup mock canvas and context
    const canvas = document.createElement('canvas');
    canvas.width = 100;
    canvas.height = 100;
    
    const mockCtx = {
      clearRect: vi.fn(),
      save: vi.fn(),
      translate: vi.fn(),
      scale: vi.fn(),
      restore: vi.fn(),
      beginPath: vi.fn(),
      moveTo: vi.fn(),
      lineTo: vi.fn(),
      fill: vi.fn(),
      stroke: vi.fn(),
    };
    
    canvas.getContext = vi.fn(() => mockCtx as any);

    // 2. Initialize renderer
    const vp = new Viewport(100, 100);
    const renderer = new LayoutRenderer(canvas, vp);

    // 3. Define shape
    const poly: IPolygon = {
      kind: 'polygon',
      id: 1,
      layer: 1,
      datatype: 0,
      points: [[0, 0], [10, 0], [10, 10], [0, 10]]
    };

    // 4. Render
    renderer.render([poly]);

    // 5. Assertions
    expect(mockCtx.clearRect).toHaveBeenCalledWith(0, 0, 100, 100);
    expect(mockCtx.save).toHaveBeenCalledTimes(1);
    
    // Viewport pan defaults to width/2, height/2 -> 50, 50
    expect(mockCtx.translate).toHaveBeenCalledWith(50, 50); 
    // Zoom defaults to 1.0, and Y is inverted
    expect(mockCtx.scale).toHaveBeenCalledWith(1, -1);
    
    expect(mockCtx.beginPath).toHaveBeenCalledTimes(1);
    expect(mockCtx.moveTo).toHaveBeenCalledWith(0, 0);
    expect(mockCtx.lineTo).toHaveBeenCalledWith(10, 0);
    expect(mockCtx.lineTo).toHaveBeenCalledWith(10, 10);
    expect(mockCtx.lineTo).toHaveBeenCalledWith(0, 10);
    expect(mockCtx.fill).toHaveBeenCalledTimes(1);
    
    expect(mockCtx.restore).toHaveBeenCalledTimes(1);
  });
});

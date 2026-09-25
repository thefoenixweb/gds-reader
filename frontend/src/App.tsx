import React, { useEffect, useRef, useState, useCallback } from 'react';
import { loadLayout, getLayers, toggleLayer, queryViewport } from './api/client';
import { ILayerInfo, IShape } from './types';
import { Viewport } from './renderer/viewport';
import { LayoutRenderer } from './renderer/canvas';

export default function App() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  
  const [layers, setLayers] = useState<ILayerInfo[]>([]);
  const layersRef = useRef<ILayerInfo[]>([]);
  
  const [viewport, setViewport] = useState<Viewport | null>(null);
  const [renderer, setRenderer] = useState<LayoutRenderer | null>(null);
  
  const isDragging = useRef(false);
  const shapesRef = useRef<IShape[]>([]);
  const abortControllerRef = useRef<AbortController | null>(null);

  // Sync layers state to ref for callbacks
  useEffect(() => {
    layersRef.current = layers;
  }, [layers]);

  // Initialize Viewport and Renderer
  useEffect(() => {
    if (!canvasRef.current || !containerRef.current) return;
    
    const canvas = canvasRef.current;
    const container = containerRef.current;
    
    canvas.width = container.clientWidth;
    canvas.height = container.clientHeight;
    
    const vp = new Viewport(canvas.width, canvas.height);
    const rnd = new LayoutRenderer(canvas, vp);
    
    rnd.init().then(() => {
      setViewport(vp);
      setRenderer(rnd);
    });

    const handleResize = () => {
      canvas.width = container.clientWidth;
      canvas.height = container.clientHeight;
      vp.width = canvas.width;
      vp.height = canvas.height;
      fetchAndDraw();
    };

    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const fastDraw = useCallback(() => {
    if (!viewport || !renderer) return;
    
    requestAnimationFrame(() => {
      renderer.render();
    });
  }, [viewport, renderer]);

  const fetchAndDraw = useCallback(() => {
    if (!viewport || !renderer || layersRef.current.length === 0) return;
    const visibleLayerIds = layersRef.current.filter(l => l.visible).map(l => l.id);
    
    const layerMap = new Map<number, ILayerInfo>();
    for (const l of layersRef.current) {
      layerMap.set(l.id, l);
    }
    renderer.layers = layerMap;

    const query = viewport.getViewportQuery();
    query.visibleLayers = visibleLayerIds;
    query.knownCells = Object.fromEntries(renderer.knownCells);
    query.maxShapes = 20000;
    
    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
    }
    const abortController = new AbortController();
    abortControllerRef.current = abortController;
    const signal = abortController.signal;
    queryViewport(query, signal).then((res) => {
      renderer.setViewportData(res);
      fastDraw();
    }).catch(err => {
      if (err.name !== 'AbortError') {
        console.error(err);
      }
    });
  }, [viewport, renderer, fastDraw]);

  // Draw when layer toggles or initialization finishes
  useEffect(() => {
    fetchAndDraw();
  }, [layers, viewport, fetchAndDraw]);

  const handleFileUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    try {
      // Clear current view
      setLayers([]);
      shapesRef.current = [];
      renderer?.clearData();
      fastDraw();
      
      await loadLayout(file);
      const fetchedLayers = await getLayers();
      
      // Auto-fit for simple.gds (which is ~400,000 DB units wide)
      if (viewport && canvasRef.current) {
        viewport.zoom = 0.0015;
        viewport.panX = 50; 
        viewport.panY = canvasRef.current.height - 50;
      }
      
      setLayers(fetchedLayers);
      (window as any).layoutLoaded = true;
      // fetchAndDraw will be triggered by layers state change
    } catch (err) {
      console.error("Upload failed", err);
      alert("Upload failed: " + err);
    }
  };

  const handleToggleLayer = async (id: number) => {
    try {
      const updatedLayer = await toggleLayer(id);
      setLayers(prev => prev.map(l => l.id === id ? updatedLayer : l));
    } catch (err) {
      console.error("Toggle layer failed", err);
    }
  };

  const handlePointerDown = (e: React.PointerEvent) => {
    isDragging.current = true;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
  };

  const handlePointerUp = (e: React.PointerEvent) => {
    isDragging.current = false;
    (e.target as HTMLElement).releasePointerCapture(e.pointerId);
    fetchAndDraw(); // Fetch new geometry for the new view area
  };

  const handlePointerMove = (e: React.PointerEvent) => {
    if (!isDragging.current || !viewport) return;
    viewport.handleMouseDrag(e.movementX, e.movementY);
    fastDraw(); // Pan visually using currently cached shapes
  };

  const handleWheel = (e: React.WheelEvent) => {
    if (!viewport) return;
    viewport.handleWheel(e.nativeEvent);
    fetchAndDraw(); 
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', fontFamily: 'sans-serif' }}>
      <div style={{ padding: '12px', borderBottom: '1px solid #ddd', background: '#fafafa', display: 'flex', alignItems: 'center' }}>
        <h2 style={{ margin: 0, fontSize: '1.2rem', marginRight: '24px' }}>GDS Layout Studio</h2>
        <input type="file" onChange={handleFileUpload} accept=".gds" />
      </div>
      
      <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
        <div style={{ width: '250px', borderRight: '1px solid #ddd', padding: '12px', overflowY: 'auto', background: '#fff' }}>
          <h3 style={{ margin: '0 0 12px 0', fontSize: '1rem' }}>Layers</h3>
          {layers.map(layer => (
            <label key={layer.id} style={{ display: 'flex', alignItems: 'center', marginBottom: '8px', cursor: 'pointer' }}>
              <input 
                type="checkbox" 
                checked={layer.visible} 
                onChange={() => handleToggleLayer(layer.id)} 
                style={{ cursor: 'pointer' }}
              />
              <span style={{ 
                display: 'inline-block', 
                width: '16px', height: '16px', 
                backgroundColor: layer.color, 
                marginLeft: '8px', marginRight: '8px',
                border: '1px solid #000'
              }}></span>
              {layer.name}
            </label>
          ))}
          {layers.length === 0 && (
            <div style={{ color: '#888', fontSize: '0.9rem' }}>No layers loaded. Upload a file first.</div>
          )}
        </div>
        
        <div ref={containerRef} style={{ flex: 1, position: 'relative', background: '#e0e0e0' }}>
          <canvas
            ref={canvasRef}
            style={{ 
              touchAction: 'none', 
              display: 'block', 
              cursor: isDragging.current ? 'grabbing' : 'grab' 
            }}
            onPointerDown={handlePointerDown}
            onPointerMove={handlePointerMove}
            onPointerUp={handlePointerUp}
            onPointerCancel={handlePointerUp}
            onWheel={handleWheel}
          />
        </div>
      </div>
    </div>
  );
}

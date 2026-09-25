import { IShape, IViewportQuery, ILayerInfo, IViewportResponse, ICellDefinition, IInstance } from '../types';

async function handleResponse<T>(res: Response): Promise<T> {
  if (!res.ok) {
    let msg = `HTTP error ${res.status}`;
    try {
      const body = await res.json();
      if (body && body.error) {
        msg = body.error;
      }
    } catch (e) {
      // Ignore JSON parse errors for non-JSON error responses
    }
    throw new Error(msg);
  }

  const text = await res.text();
  if (!text) return undefined as any;
  return JSON.parse(text);
}

export async function loadLayout(file: File): Promise<void> {
  const formData = new FormData();
  formData.append('file', file);

  const res = await fetch('http://localhost:3000/api/layout/load', {
    method: 'POST',
    body: formData,
  });

  await handleResponse(res);
}

export async function queryViewport(query: IViewportQuery, signal?: AbortSignal): Promise<IViewportResponse> {
  const res = await fetch('http://localhost:3000/api/layout/query', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(query),
    ...(signal ? { signal } : {}),
  });

  if (!res.ok) {
    throw new Error(`HTTP error ${res.status}`);
  }

  const buffer = await res.arrayBuffer();
  const view = new DataView(buffer);

  if (buffer.byteLength < 5) return { shapes: [], instances: [], cellDefinitions: {}, lodLevel: 'shape' };

  let offset = 0;
  const lodByte = view.getUint8(offset); offset += 1;
  const lodLevel = lodByte === 0 ? 'cell' : lodByte === 1 ? 'block' : 'shape';

  const readString = (): string => {
    const len = view.getUint32(offset, true); offset += 4;
    if (len > 10000) throw new Error(`String too long: ${len} at offset ${offset}`);
    const bytes = new Uint8Array(buffer, offset, len);
    offset += len;
    return new TextDecoder().decode(bytes);
  };

  const readShapes = (): IShape[] => {
    const numShapes = view.getUint32(offset, true); offset += 4;
    const shapes: IShape[] = [];
    for (let i = 0; i < numShapes; i++) {
      const kind = view.getUint8(offset); offset += 1;
      if (kind === 0) {
        const layer = view.getUint16(offset, true); offset += 2;
        const datatype = view.getUint16(offset, true); offset += 2;
        const numPoints = view.getUint32(offset, true); offset += 4;
        
        if (numPoints > 50000000) throw new Error(`Corrupt stream: numPoints ${numPoints} at offset ${offset}`);
        
        const points: [number, number][] = [];
        for (let p = 0; p < numPoints; p++) {
          const x = view.getFloat64(offset, true); offset += 8;
          const y = view.getFloat64(offset, true); offset += 8;
          points.push([x, y]);
        }
        shapes.push({ kind: 'polygon', id: i, layer, datatype, points } as any);
      } else if (kind === 1) {
        const layer = view.getUint16(offset, true); offset += 2;
        const datatype = view.getUint16(offset, true); offset += 2;
        const width = view.getFloat64(offset, true); offset += 8;
        const pathType = view.getUint8(offset); offset += 1;
        const numPoints = view.getUint32(offset, true); offset += 4;
        
        if (numPoints > 50000000) throw new Error(`Corrupt stream: numPoints ${numPoints} at offset ${offset}`);
        
        const points: [number, number][] = [];
        for (let p = 0; p < numPoints; p++) {
          const x = view.getFloat64(offset, true); offset += 8;
          const y = view.getFloat64(offset, true); offset += 8;
          points.push([x, y]);
        }
        shapes.push({ kind: 'path', id: i, layer, datatype, width, pathType, points } as any);
      } else {
        throw new Error(`Unknown shape kind: ${kind} at offset ${offset - 1}`);
      }
    }
    return shapes;
  };

  const readInstances = (): IInstance[] => {
    const numInsts = view.getUint32(offset, true); offset += 4;
    const instances: IInstance[] = [];
    for (let i = 0; i < numInsts; i++) {
      const cellName = readString();
      const transform = [
        view.getFloat64(offset, true),
        view.getFloat64(offset + 8, true),
        view.getFloat64(offset + 16, true),
        view.getFloat64(offset + 24, true),
        view.getFloat64(offset + 32, true),
        view.getFloat64(offset + 40, true),
      ] as [number, number, number, number, number, number];
      offset += 48;
      const cols = view.getUint16(offset, true); offset += 2;
      const rows = view.getUint16(offset, true); offset += 2;
      const colVector: [number, number] = [view.getFloat64(offset, true), view.getFloat64(offset + 8, true)]; offset += 16;
      const rowVector: [number, number] = [view.getFloat64(offset, true), view.getFloat64(offset + 8, true)]; offset += 16;
      
      instances.push({ cellName, transform, cols, rows, colVector, rowVector });
    }
    return instances;
  };

  const shapes = readShapes();
  const instances = readInstances();

  const numDefs = view.getUint32(offset, true); offset += 4;
  const cellDefinitions: Record<string, ICellDefinition> = {};
  for (let i = 0; i < numDefs; i++) {
    const name = readString();
    
    // bbox
    const bbox: [number, number, number, number] = [
      view.getFloat64(offset, true),
      view.getFloat64(offset + 8, true),
      view.getFloat64(offset + 16, true),
      view.getFloat64(offset + 24, true),
    ];
    offset += 32;
    
    const defShapes = readShapes();
    const defInstances = readInstances();
    cellDefinitions[name] = { bbox, shapes: defShapes, instances: defInstances };
  }

  return { shapes, instances, cellDefinitions, lodLevel };
}

export async function getLayers(): Promise<ILayerInfo[]> {
  const res = await fetch('http://localhost:3000/api/layers');
  return handleResponse<ILayerInfo[]>(res);
}

export async function toggleLayer(id: number): Promise<ILayerInfo> {
  const res = await fetch(`http://localhost:3000/api/layers/${id}/toggle`, {
    method: 'PATCH',
  });

  return handleResponse<ILayerInfo>(res);
}

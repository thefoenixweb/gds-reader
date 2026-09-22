import { IShape, IViewportQuery, ILayerInfo, IViewportResponse } from '../types';

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

export async function queryViewport(query: IViewportQuery): Promise<IShape[]> {
  const res = await fetch('http://localhost:3000/api/layout/query', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(query),
  });

  const data = await handleResponse<IViewportResponse>(res);
  return data.shapes;
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

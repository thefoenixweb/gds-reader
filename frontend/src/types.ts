export type IPoint = [number, number];
export interface IBoundingBox  { x1: number; y1: number; x2: number; y2: number; }

export interface IPolygon {
  readonly kind: "polygon";
  id: number;
  layer: number;
  datatype: number;
  points: IPoint[];
}

export interface IPath {
  readonly kind: "path";
  id: number;
  layer: number;
  datatype: number;
  points: IPoint[];
  width: number;
  pathType: 0 | 1 | 2;
}

export interface IText {
  readonly kind: "text";
  id: number;
  layer: number;
  texttype: number;
  position: IPoint;
  string: string;
  rotationDeg: number;
  magnification: number;
  mirrorX: boolean;
}

export type IShape = IPolygon | IPath | IText;

export interface ILayerInfo {
  id: number;
  name: string;
  color: string;
  visible: boolean;
}

export interface ILayoutMeta {
  libName: string;
  topCell: string | null;
  cellCount: number;
  flatShapeCount: number;
  dbUnitInMeters: number;
  userUnitInMeters: number;
}

export interface IViewportQuery {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  zoom: number;
  visibleLayers: number[];
  knownCells: Record<string, "cell" | "block" | "shape">;
  maxShapes?: number;
}

export interface IInstance {
  cellName: string;
  transform: [number, number, number, number, number, number]; // m11, m12, tx, m21, m22, ty
  cols: number;
  rows: number;
  colVector: [number, number];
  rowVector: [number, number];
}

export interface ICellDefinition {
  bbox: [number, number, number, number];
  shapes: IShape[];
  instances: IInstance[];
}

export interface IViewportResponse {
  shapes: IShape[];
  instances: IInstance[];
  cellDefinitions: Record<string, ICellDefinition>;
  lodLevel: "cell" | "block" | "shape";
}

export interface IEditEvent {
  type: "add" | "delete" | "move";
  shape: IShape;
  timestamp: number;
  originatorId: string;
}

export interface IConnectedUsersEvent {
  readonly type: "connected_users";
  count: number;
}

export type IWsMessage = IEditEvent | IConnectedUsersEvent;

export function isEditEvent(msg: IWsMessage): msg is IEditEvent {
  return msg.type === "add" || msg.type === "delete" || msg.type === "move";
}

export function isConnectedUsersEvent(msg: IWsMessage): msg is IConnectedUsersEvent {
  return msg.type === "connected_users";
}

export function isPolygon(shape: IShape): shape is IPolygon { return shape.kind === "polygon"; }
export function isPath(shape: IShape): shape is IPath       { return shape.kind === "path"; }
export function isText(shape: IShape): shape is IText       { return shape.kind === "text"; }

use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use gds_parser::{parser::GdsParser, reader::GdsReader};
use layout_db::index::ShapeIndex;
use serde_json::{json, Value};
use std::io::Cursor;
use crate::query::{handle_query, ViewportQuery, ViewportResponse};
use crate::state::AppState;

type ApiError = (StatusCode, Json<Value>);

fn err(status: StatusCode, msg: impl Into<String>) -> ApiError {
    (status, Json(json!({ "error": msg.into() })))
}

use axum::extract::DefaultBodyLimit;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/layout/load",  post(load_layout))
        .route("/api/layout/query", post(query_layout))
        .layer(DefaultBodyLimit::disable())
}

pub async fn load_layout(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<StatusCode, ApiError> {
    let mut gds_bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| err(StatusCode::BAD_REQUEST, e.to_string()))?
    {
        gds_bytes = Some(
            field
                .bytes()
                .await
                .map_err(|e| err(StatusCode::BAD_REQUEST, e.to_string()))?
                .to_vec(),
        );
        break;
    }

    let bytes = gds_bytes
        .ok_or_else(|| err(StatusCode::BAD_REQUEST, "no file in request"))?;

    let layout = GdsParser::new(GdsReader::new(Cursor::new(bytes)))
        .parse()
        .map_err(|e| err(StatusCode::BAD_REQUEST, e.to_string()))?;

    let index = ShapeIndex::build_from_layout(&layout);
    *state.layout.write().await = Some(layout);
    *state.index.write().await  = Some(index);

    Ok(StatusCode::OK)
}

pub async fn query_layout(
    State(state): State<AppState>,
    Json(query): Json<ViewportQuery>,
) -> Result<Json<ViewportResponse>, ApiError> {
    let guard = state.index.read().await;
    let index = guard
        .as_ref()
        .ok_or_else(|| err(StatusCode::SERVICE_UNAVAILABLE, "No layout loaded"))?;

    Ok(Json(handle_query(index, &query)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use layout_db::cell::{Cell, Layout, Shape};
    use layout_db::index::ShapeIndex;

    async fn loaded_state() -> AppState {
        let state = AppState::new();
        let mut layout = Layout::new("TEST");
        let mut cell = Cell::new("TOP");
        cell.shapes.push(Shape::Polygon {
            layer: 1, datatype: 0,
            points: vec![(0.,0.),(10.,0.),(10.,10.),(0.,10.),(0.,0.)],
        });
        cell.shapes.push(Shape::Polygon {
            layer: 2, datatype: 0,
            points: vec![(0.,0.),(5.,0.),(5.,5.),(0.,5.),(0.,0.)],
        });
        layout.insert_cell(cell);
        let index = ShapeIndex::build_from_layout(&layout);
        *state.layout.write().await = Some(layout);
        *state.index.write().await  = Some(index);
        state
    }

    fn vp(x1: f64, y1: f64, x2: f64, y2: f64, layers: Vec<u16>) -> ViewportQuery {
        ViewportQuery { x1, y1, x2, y2, zoom: 10.0, visible_layers: layers }
    }

    #[tokio::test]
    async fn query_before_load_returns_503() {
        let state = AppState::new();
        let q = vp(0., 0., 100., 100., vec![]);
        let result = query_layout(State(state), Json(q)).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().0, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn query_after_load_returns_shapes() {
        let state = loaded_state().await;
        let result = query_layout(State(state), Json(vp(0., 0., 20., 20., vec![]))).await.unwrap();
        assert_eq!(result.0.shapes.len(), 2);
    }

    #[tokio::test]
    async fn query_layer_filter_applied() {
        let state = loaded_state().await;
        let result = query_layout(State(state), Json(vp(0., 0., 20., 20., vec![1]))).await.unwrap();
        assert_eq!(result.0.shapes.len(), 1);
    }

    #[tokio::test]
    async fn query_empty_viewport_returns_no_shapes() {
        let state = loaded_state().await;
        let result = query_layout(State(state), Json(vp(500., 500., 600., 600., vec![]))).await.unwrap();
        assert!(result.0.shapes.is_empty());
    }

    #[tokio::test]
    async fn load_then_query_updates_state() {
        let state = AppState::new();
        let state2 = state.clone();

        let mut layout = Layout::new("L");
        let mut cell = Cell::new("C");
        cell.shapes.push(Shape::Polygon {
            layer: 3, datatype: 0,
            points: vec![(0.,0.),(1.,0.),(1.,1.),(0.,1.),(0.,0.)],
        });
        layout.insert_cell(cell);
        let index = ShapeIndex::build_from_layout(&layout);
        *state.layout.write().await = Some(layout);
        *state.index.write().await  = Some(index);

        let result = query_layout(State(state2), Json(vp(0., 0., 5., 5., vec![]))).await.unwrap();
        assert_eq!(result.0.shapes.len(), 1);
    }

    #[tokio::test]
    async fn error_body_is_valid_json_with_error_key() {
        let state = AppState::new();
        let (_, json_body) = query_layout(State(state), Json(vp(0., 0., 1., 1., vec![]))).await.unwrap_err();
        assert!(json_body.0.get("error").is_some());
    }
}

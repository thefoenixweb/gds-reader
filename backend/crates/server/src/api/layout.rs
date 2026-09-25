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
use crate::query::{handle_query, ViewportQuery};
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

use axum::response::IntoResponse;

pub async fn query_layout(
    State(state): State<AppState>,
    Json(query): Json<ViewportQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let index_guard = state.index.read().await;
    let layout_guard = state.layout.read().await;
    
    let index = index_guard
        .as_ref()
        .ok_or_else(|| err(StatusCode::SERVICE_UNAVAILABLE, "No index loaded"))?;
    
    let layout = layout_guard.as_ref();

    let response = handle_query(index, layout, &query);
    let bytes = response.to_binary();
    Ok((
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
        bytes,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use layout_db::cell::{Cell, Layout, Shape};
    use layout_db::index::ShapeIndex;
    use crate::query::handle_query;

    async fn loaded_state() -> AppState {
        let state = AppState::new();
        let mut layout = Layout::new("TEST");
        let mut cell = Cell::new("TOP");
        cell.shapes.push(Shape::Polygon {
            layer: 1,
            datatype: 0,
            points: vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
        });
        cell.shapes.push(Shape::Polygon {
            layer: 2,
            datatype: 0,
            points: vec![(20.0, 20.0), (30.0, 20.0), (30.0, 30.0), (20.0, 30.0)],
        });
        layout.insert_cell(cell);

        let index = ShapeIndex::build_from_layout(&layout);
        *state.layout.write().await = Some(layout);
        *state.index.write().await = Some(index);
        state
    }

    fn vp(x1: f64, y1: f64, x2: f64, y2: f64, visible_layers: Vec<u16>) -> ViewportQuery {
        ViewportQuery {
            x1,
            y1,
            x2,
            y2,
            zoom: 1.0,
            visible_layers,
        }
    }

    #[tokio::test]
    async fn query_before_load_returns_503() {
        let state = AppState::new();
        let result = query_layout(State(state), Json(vp(0., 0., 1., 1., vec![]))).await;
        // 503 is returned by `unwrap_err`
        assert_eq!(result.map(|_| ()).unwrap_err().0, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn query_after_load_returns_shapes() {
        let state = loaded_state().await;
        let query = vp(0., 0., 40., 40., vec![]);
        let guard = state.index.read().await;
        let index = guard.as_ref().unwrap();
        let result = handle_query(index, &query);
        assert_eq!(result.shapes.len(), 2);
    }

    #[tokio::test]
    async fn query_layer_filter_applied() {
        let state = loaded_state().await;
        let query = vp(0., 0., 40., 40., vec![1]);
        let guard = state.index.read().await;
        let index = guard.as_ref().unwrap();
        let result = handle_query(index, &query);
        assert_eq!(result.shapes.len(), 1);
        match &result.shapes[0] {
            crate::query::ShapeDto::Polygon { layer, .. } => assert_eq!(*layer, 1),
            _ => panic!("Expected polygon"),
        }
    }

    #[tokio::test]
    async fn query_empty_viewport_returns_no_shapes() {
        let state = loaded_state().await;
        let query = vp(100., 100., 110., 110., vec![]);
        let guard = state.index.read().await;
        let index = guard.as_ref().unwrap();
        let result = handle_query(index, &query);
        assert!(result.shapes.is_empty());
    }

    #[tokio::test]
    async fn load_then_query_updates_state() {
        let state = AppState::new();
        let mut layout = Layout::new("L2");
        let mut cell = Cell::new("TOP2");
        cell.shapes.push(Shape::Polygon { layer: 99, datatype: 0, points: vec![(0.0,0.0), (1.0,1.0)] });
        layout.insert_cell(cell);
        let index = ShapeIndex::build_from_layout(&layout);
        *state.layout.write().await = Some(layout);
        *state.index.write().await = Some(index);
        
        let query = vp(-10., -10., 10., 10., vec![]);
        let guard = state.index.read().await;
        let idx = guard.as_ref().unwrap();
        let result = handle_query(idx, &query);
        assert_eq!(result.shapes.len(), 1);
    }
    
    #[tokio::test]
    async fn error_body_is_valid_json_with_error_key() {
        let state = AppState::new();
        let result = query_layout(State(state), Json(vp(0., 0., 1., 1., vec![]))).await;
        let (_, json_body) = result.map(|_| ()).unwrap_err();
        assert_eq!(json_body.0["error"], "No layout loaded");
    }
}

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, patch},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::state::AppState;
use layout_db::cell::LayerInfo;

type ApiError = (StatusCode, Json<Value>);

fn err(status: StatusCode, msg: impl Into<String>) -> ApiError {
    (status, Json(json!({ "error": msg.into() })))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerDto {
    pub id: u16,
    pub name: String,
    pub color: String,
    pub visible: bool,
}

impl From<&LayerInfo> for LayerDto {
    fn from(info: &LayerInfo) -> Self {
        LayerDto {
            id: info.id,
            name: info.name.clone(),
            color: info.color.clone(),
            visible: info.visible,
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/layers", get(get_layers))
        .route("/api/layers/:id/toggle", patch(toggle_layer))
}

pub async fn get_layers(
    State(state): State<AppState>,
) -> Result<Json<Vec<LayerDto>>, ApiError> {
    let guard = state.layout.read().await;
    let layout = guard
        .as_ref()
        .ok_or_else(|| err(StatusCode::SERVICE_UNAVAILABLE, "No layout loaded"))?;

    let mut layers: Vec<LayerDto> = layout.layers.values().map(LayerDto::from).collect();
    layers.sort_by_key(|l| l.id);
    
    Ok(Json(layers))
}

pub async fn toggle_layer(
    State(state): State<AppState>,
    Path(id): Path<u16>,
) -> Result<Json<LayerDto>, ApiError> {
    let mut guard = state.layout.write().await;
    let layout = guard
        .as_mut()
        .ok_or_else(|| err(StatusCode::SERVICE_UNAVAILABLE, "No layout loaded"))?;

    let layer = layout
        .layers
        .get_mut(&id)
        .ok_or_else(|| err(StatusCode::NOT_FOUND, format!("Layer {} not found", id)))?;

    layer.visible = !layer.visible;
    Ok(Json(LayerDto::from(&*layer)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use layout_db::cell::Layout;

    async fn loaded_state() -> AppState {
        let state = AppState::new();
        let mut layout = Layout::new("TEST");
        layout.register_layer(1);
        layout.register_layer(2);
        *state.layout.write().await = Some(layout);
        state
    }

    #[tokio::test]
    async fn get_layers_returns_all_layers() {
        let state = loaded_state().await;
        let response = get_layers(State(state)).await.unwrap();
        assert_eq!(response.0.len(), 2);
        assert_eq!(response.0[0].id, 1);
        assert_eq!(response.0[1].id, 2);
    }

    #[tokio::test]
    async fn toggle_layer_flips_visibility() {
        let state = loaded_state().await;
        
        let mut layout = state.layout.write().await;
        assert!(layout.as_ref().unwrap().layers.get(&1).unwrap().visible);
        drop(layout);

        let response = toggle_layer(State(state.clone()), Path(1)).await.unwrap();
        assert!(!response.0.visible);

        let layout = state.layout.read().await;
        assert!(!layout.as_ref().unwrap().layers.get(&1).unwrap().visible);
    }

    #[tokio::test]
    async fn toggle_layer_returns_404_if_not_found() {
        let state = loaded_state().await;
        let response = toggle_layer(State(state), Path(99)).await;
        assert!(response.is_err());
        assert_eq!(response.unwrap_err().0, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn endpoints_fail_if_no_layout() {
        let state = AppState::new();
        
        let res1 = get_layers(State(state.clone())).await;
        assert_eq!(res1.unwrap_err().0, StatusCode::SERVICE_UNAVAILABLE);

        let res2 = toggle_layer(State(state.clone()), Path(1)).await;
        assert_eq!(res2.unwrap_err().0, StatusCode::SERVICE_UNAVAILABLE);
    }
}

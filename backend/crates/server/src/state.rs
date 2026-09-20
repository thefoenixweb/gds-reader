use std::sync::Arc;
use tokio::sync::RwLock;
use layout_db::cell::Layout;
use layout_db::index::ShapeIndex;

#[derive(Clone)]
pub struct AppState {
    pub layout: Arc<RwLock<Option<Layout>>>,
    pub index:  Arc<RwLock<Option<ShapeIndex>>>,
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            layout: Arc::new(RwLock::new(None)),
            index:  Arc::new(RwLock::new(None)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_state_layout_is_none() {
        let state = AppState::new();
        assert!(state.layout.read().await.is_none());
    }

    #[tokio::test]
    async fn new_state_index_is_none() {
        let state = AppState::new();
        assert!(state.index.read().await.is_none());
    }

    #[tokio::test]
    async fn clone_shares_layout_arc() {
        let state = AppState::new();
        let clone = state.clone();
        *state.layout.write().await = Some(Layout::new("TEST"));
        assert!(clone.layout.read().await.is_some());
    }

    #[tokio::test]
    async fn clone_shares_index_arc() {
        let state = AppState::new();
        let clone = state.clone();
        let layout = Layout::new("X");
        *state.index.write().await = Some(ShapeIndex::build_from_layout(&layout));
        assert!(clone.index.read().await.is_some());
    }

    #[tokio::test]
    async fn write_then_read_layout_name() {
        let state = AppState::new();
        *state.layout.write().await = Some(Layout::new("MYLIB"));
        let guard = state.layout.read().await;
        assert_eq!(guard.as_ref().unwrap().lib_name, "MYLIB");
    }

    #[tokio::test]
    async fn replace_layout_updates_all_clones() {
        let state = AppState::new();
        let a = state.clone();
        let b = state.clone();
        *state.layout.write().await = Some(Layout::new("V1"));
        assert_eq!(a.layout.read().await.as_ref().unwrap().lib_name, "V1");
        *state.layout.write().await = Some(Layout::new("V2"));
        assert_eq!(b.layout.read().await.as_ref().unwrap().lib_name, "V2");
    }
}

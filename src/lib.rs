mod accountant;
mod assistant;
mod core;
mod provider;
mod reception;
mod storage;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::Router;

use std::sync::Arc;
use tower_http::trace::TraceLayer;

// 统筹全部逻辑的应用Agent
pub use reception::Config;
use reception::{Agent, UrlVerifyParams, UserMsgParams};

// Shared state used in all routers
type SharedState = Arc<AppState>;

struct AppState {
    app_agent: Agent,
}

pub fn app(config: &Config) -> Router {
    // 初始化APP agent。
    let mut cfg: Config = config.clone();
    let app_agent = match Agent::new(&mut cfg) {
        Err(e) => panic!("初始化应用错误：{e}"),
        Ok(agent) => agent,
    };

    // Init a router with this shared state.
    let state = Arc::new(AppState { app_agent });

    Router::new()
        .route("/verify/agent/:agent_id", post(user_msg_handler))
        .route("/chat/agent/:agent_id", get(server_verification_handler))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

// 响应腾讯服务器的可用性验证请求
async fn server_verification_handler(
    Path(agent_id): Path<u64>,
    State(state): State<SharedState>,
    params: Query<UrlVerifyParams>,
) -> Result<String, StatusCode> {
    tracing::debug!("Got url verification request.");

    state.app_agent.verify_url(agent_id, params)
}

// 响应用户发来的消息
async fn user_msg_handler(
    Path(agent_id): Path<u64>,
    State(state): State<SharedState>,
    params: Query<UserMsgParams>,
    body: String,
) -> StatusCode {
    tracing::debug!("Got user message.");

    // 微信服务器要求即时响应，故异步处理这条消息。
    tokio::spawn(async move {
        state
            .app_agent
            .handle_user_request(agent_id, params, body)
            .await;
    });

    StatusCode::OK
}

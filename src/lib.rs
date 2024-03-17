mod reception;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;

use std::sync::Arc;
use tower_http::trace::TraceLayer;

// 统筹全部逻辑的应用Agent
use reception::{Agent, UrlVerifyParams, UserMsgParams};

// Shared state used in all routers
type SharedState = Arc<AppState>;

struct AppState {
    app_agent: Agent,
}

pub fn app(
    app_token: &str,
    b64encoded_aes_key: &str,
    corp_id: &str,
    secret: &str,
    provider_id: i32,
    oai_key: &str,
    db_path: &str,
) -> Router {
    // 初始化APP agent。
    let app_agent = match Agent::new(
        app_token,
        b64encoded_aes_key,
        corp_id,
        secret,
        provider_id,
        oai_key,
        db_path,
    ) {
        Ok(agent) => agent,
        Err(e) => panic!("初始化应用错误：{e}"),
    };

    // Init a router with this shared state.
    let state = Arc::new(AppState { app_agent });

    Router::new()
        .route("/", get(server_verification_handler).post(user_msg_handler))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

// 响应腾讯服务器的可用性验证请求
async fn server_verification_handler(
    State(state): State<SharedState>,
    params: Query<UrlVerifyParams>,
) -> Result<String, StatusCode> {
    tracing::debug!("Got url verification request.");

    state.app_agent.verify_url(params)
}

// 响应用户发来的消息
async fn user_msg_handler(
    State(state): State<SharedState>,
    params: Query<UserMsgParams>,
    body: String,
) -> StatusCode {
    tracing::debug!("Got user message.");

    // 微信服务器要求即时响应，故异步处理这条消息。
    tokio::spawn(async move {
        state.app_agent.handle_user_request(params, body).await;
    });

    StatusCode::OK
}

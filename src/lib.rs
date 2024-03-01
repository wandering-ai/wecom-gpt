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

pub async fn app(
    app_token: &str,
    b64encoded_aes_key: &str,
    corp_id: &str,
    secret: &str,
    oai_endpoint: &str,
    oai_key: &str,
) -> Router {
    // 初始化APP agent。
    let app_agent = Agent::new(
        app_token,
        b64encoded_aes_key,
        corp_id,
        secret,
        oai_endpoint,
        oai_key,
    );

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
    state.app_agent.verify_url(params)
}

// 响应用户发来的消息
async fn user_msg_handler(
    State(state): State<SharedState>,
    params: Query<UserMsgParams>,
    body: String,
) -> StatusCode {
    // 微信服务器要求即时响应，故异步处理这条消息。
    tokio::spawn(async move {
        state.app_agent.handle_user_request(params, body).await;
    });

    StatusCode::OK
}

use axum::extract::Query;
use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;

use serde::Deserialize;
use tower_http::trace::TraceLayer;

/// 请求涉及到的公共参数
#[derive(Deserialize)]
struct Params {
    msg_signature: String,
    timestamp: u64,
    nonce: String,
    echostr: String,
}

/// 响应腾讯服务器的可用性验证请求。
async fn server_verification_handler(params: Query<Params>) -> Result<String, StatusCode> {
    if params.timestamp > 0 {
        tracing::info!(
            "signature: {}, timestamp: {}, nonce: {}, echostr: {}",
            params.msg_signature,
            params.timestamp,
            params.nonce,
            params.echostr
        );
        Ok(format!("Message: {}", params.echostr))
    } else {
        tracing::error!("Error! Code: {}", StatusCode::BAD_REQUEST);
        Err(StatusCode::BAD_REQUEST)
    }
}

/// 处理用户发来的消息。
async fn user_msg_handler() {}

#[tokio::main]
async fn main() {
    // Setup tracing
    tracing_subscriber::fmt().compact().init();

    // Build our application
    let app = Router::new()
        .route("/", get(server_verification_handler).post(user_msg_handler))
        .layer(TraceLayer::new_for_http());

    // run our app with hyper, listening globally on port 8088
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8088").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

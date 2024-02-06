use axum::extract::Query;
use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;

use serde::Deserialize;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() {
    // Setup tracing
    tracing_subscriber::fmt().compact().init();

    // run our app with hyper, listening globally on port 8088
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8088").await.unwrap();
    axum::serve(listener, app()).await.unwrap();
}

fn app() -> Router {
    Router::new()
        .route("/", get(server_verification_handler).post(user_msg_handler))
        .layer(TraceLayer::new_for_http())
}

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

// 单元测试
#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn server_verification() {
        let app = app();
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/?msg_signature=abc&timestamp=1986&nonce=xyz&echostr=hello")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body();
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        assert_eq!(&bytes[..], b"Message: hello");
    }
}

use axum::extract::Query;
use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;

use serde::Deserialize;
use tower_http::trace::TraceLayer;

// 企业微信加解密模块
mod crypto;

pub fn app() -> Router {
    Router::new()
        .route("/", get(server_verification_handler).post(user_msg_handler))
        .layer(TraceLayer::new_for_http())
}

/// 请求涉及到的公共参数
#[derive(Deserialize)]
struct Params {
    msg_signature: String,
    timestamp: String,
    nonce: String,
    echostr: String,
}

/// 响应腾讯服务器的可用性验证请求。
async fn server_verification_handler(params: Query<Params>) -> Result<String, StatusCode> {
    if crypto::generate_signature(vec![&params.timestamp, &params.nonce, "c", &params.echostr])
        == params.msg_signature
    {
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
                    .uri("/?msg_signature=a8addbc99f8b3f51d2adbceb605d650b9a8940e2&timestamp=0&nonce=a&echostr=b")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body();
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        assert_eq!(&bytes[..], b"Message: b");
    }
}

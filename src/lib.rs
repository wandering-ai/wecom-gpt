use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;

use serde::Deserialize;
use tower_http::trace::TraceLayer;

// 企业微信加解密模块
mod crypto;

#[derive(Clone)]
struct AppState {
    app_token: String,
    agent: crypto::CryptoAgent,
}

pub fn app(app_token: &str, encoding_aes_key: &str) -> Router {
    let state = AppState {
        app_token: String::from(app_token),
        agent: crypto::CryptoAgent::new(encoding_aes_key),
    };
    Router::new()
        .route("/", get(server_verification_handler).post(user_msg_handler))
        .with_state(state)
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
async fn server_verification_handler(
    State(state): State<AppState>,
    params: Query<Params>,
) -> Result<String, StatusCode> {
    // Is this request safe?
    if crypto::generate_signature(vec![
        &params.timestamp,
        &params.nonce,
        &state.app_token,
        &params.echostr,
    ]) != params.msg_signature
    {
        tracing::error!("Error! Code: {}", StatusCode::BAD_REQUEST);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Give the server what it expects.
    match state.agent.decrypt(&params.echostr) {
        Ok(s) => Ok(s.text),
        Err(e) => {
            tracing::error!("Error!: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
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
        let app_token = "HWmSYaJCKJFVn9YvbdVEmiYl";
        let encoding_aes_key = "cGCVnNJRgRu6wDgo7gxG2diBovGnRQq1Tqy4Rm4V4qF";
        let app = app(app_token, encoding_aes_key);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/?msg_signature=5666e50620f9616a29c109400608107cf22f440c&timestamp=1707492633&nonce=1cb5ep5w0z5&echostr=yIPnqi0lsdTE1XZUNQR5EtlSSzrdTqC2WjN1IgKaBBIrofmwjciHCqTn6grIcaLw2%2FMDGi7DsGHp%2FibRx0n8Fg%3D%3D")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body();
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        assert_eq!(&bytes[..], b"01234567890109876543210");
    }
}

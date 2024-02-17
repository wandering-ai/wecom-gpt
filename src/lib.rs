use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;

use serde::{Deserialize, Serialize};
use serde_xml_rs::from_str;
use tower_http::trace::TraceLayer;

// 企业微信加解密模块
use wecom_crypto::{generate_signature, CryptoAgent};

#[derive(Clone)]
struct AppState {
    app_token: String,
    agent: CryptoAgent,
}

pub fn app(app_token: &str, b64encoded_aes_key: &str) -> Router {
    let state = AppState {
        app_token: String::from(app_token),
        agent: CryptoAgent::new(b64encoded_aes_key),
    };
    Router::new()
        .route("/", get(server_verification_handler).post(user_msg_handler))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

/// 服务器的可用性验证请求涉及到的参数
#[derive(Deserialize)]
struct UrlVerifyParams {
    msg_signature: String,
    timestamp: String,
    nonce: String,
    echostr: String,
}

/// 响应腾讯服务器的可用性验证请求。
async fn server_verification_handler(
    State(state): State<AppState>,
    params: Query<UrlVerifyParams>,
) -> Result<String, StatusCode> {
    // Is this request safe?
    if generate_signature(vec![
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
            tracing::error!("Error in decrypting: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 用户主动发送来的消息涉及到的参数
#[derive(Deserialize)]
struct UserMsgParams {
    msg_signature: String,
    nonce: String,
    timestamp: String,
}

// 请求Body结构体
// <xml>
//   <ToUserName><![CDATA[toUser]]></ToUserName>
//   <AgentID><![CDATA[toAgentID]]></AgentID>
//   <Encrypt><![CDATA[msg_encrypt]]></Encrypt>
// </xml>
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct RequestBody {
    #[serde(rename = "ToUserName")]
    to_user_name: String,
    #[serde(rename = "AgentID")]
    agent_id: String,
    #[serde(rename = "Encrypt")]
    encrypted_str: String,
}

/// 处理用户发来的消息。
async fn user_msg_handler(
    State(state): State<AppState>,
    params: Query<UserMsgParams>,
    body: String,
) -> Result<String, StatusCode> {
    // Handle the request.
    let body: RequestBody = from_str(&body).unwrap();

    // Is this request safe?
    if generate_signature(vec![
        &params.timestamp,
        &params.nonce,
        &state.app_token,
        &body.encrypted_str,
    ]) != params.msg_signature
    {
        tracing::error!("Error checking signature. The request is unsafe.");
        return Err(StatusCode::BAD_REQUEST);
    }

    // Decrypt the message
    dbg!(body);
    let msg = state.agent.decrypt(&body.encrypted_str).unwrap();
    tracing::info!(msg.text);
    Ok(msg.text)
}

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

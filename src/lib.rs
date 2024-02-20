use std::sync::{Arc, RwLock};
use std::thread::sleep;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;

use serde::{Deserialize, Serialize};
use serde_xml_rs::from_str;
use tower_http::trace::TraceLayer;

// 企业微信加解密模块
use wecom_crypto::{generate_signature, CryptoAgent};

// 企业微信API模块
use wecom_agent::{TextMsg, TextMsgContent, WecomAgent};

// WecomAgent为共享对象。当access_token更新时，需要避免数据冲突。
type SharedState = Arc<RwLock<AppState>>;

#[derive(Clone)]
struct AppState {
    app_token: String,
    crypto_agent: CryptoAgent,
    wecom_agent: WecomAgent,
}

pub async fn app(app_token: &str, b64encoded_aes_key: &str, corp_id: &str, secret: &str) -> Router {
    // Try init the wecom agent. Internet connection is required for fetching
    // access token from WeCom server, meaning this may fail.
    let mut wecom_agent = WecomAgent::new(corp_id, secret);
    for count in [1, 2, 3] {
        match wecom_agent.update_token().await {
            Ok(_) => break,
            Err(e) => tracing::error!("Token update error in try {}: {}", count, e),
        }
        sleep(Duration::from_secs(1));
    }
    if !wecom_agent.token_is_some() {
        panic!("Failed to fetch access token. Are the corpid and secret valid?");
    }

    // Init a router with this shared state.
    let state = SharedState::new(
        AppState {
            app_token: String::from(app_token),
            crypto_agent: CryptoAgent::new(b64encoded_aes_key),
            wecom_agent,
        }
        .into(),
    );
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
    State(state): State<SharedState>,
    params: Query<UrlVerifyParams>,
) -> Result<String, StatusCode> {
    // Lock the state
    let state = state.read();
    if state.is_err() {
        tracing::error!("Can not lock the app state.");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let state = state.unwrap();

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
    match state.crypto_agent.decrypt(&params.echostr) {
        Ok(t) => Ok(t.text),
        Err(e) => {
            tracing::error!("Error in decrypting: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
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

// 存储用户所发送消息的结构体
// <xml>
//   <ToUserName><![CDATA[ww637951f75e40d82b]]></ToUserName>
//   <FromUserName><![CDATA[YinGuoBing]]></FromUserName>
//   <CreateTime>1708218294</CreateTime>
//   <MsgType><![CDATA[text]]></MsgType>
//   <Content><![CDATA[[呲牙]]]></Content>
//   <MsgId>7336741709953816625</MsgId>
//   <AgentID>1000002</AgentID>
// </xml>
#[derive(Debug, Deserialize, PartialEq)]
struct ReceivedMsg {
    #[serde(rename = "ToUserName")]
    to_user_name: String,
    #[serde(rename = "FromUserName")]
    from_user_name: String,
    #[serde(rename = "CreateTime")]
    create_time: usize,
    #[serde(rename = "MsgType")]
    msg_type: String,
    #[serde(rename = "Content")]
    content: String,
    #[serde(rename = "MsgId")]
    msg_id: String,
    #[serde(rename = "AgentID")]
    agent_id: String,
}

/// 处理用户发来的消息。
async fn user_msg_handler(
    State(state): State<SharedState>,
    params: Query<UserMsgParams>,
    body: String,
) -> StatusCode {
    // Handle the request.
    let body: RequestBody = from_str(&body).unwrap();

    let mut received_msg: ReceivedMsg;
    {
        // Acquire the lock
        let state = state.read();
        if state.is_err() {
            tracing::error!("Can not lock the app state.");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
        let state = state.unwrap();

        // Is this request safe?
        if generate_signature(vec![
            &params.timestamp,
            &params.nonce,
            &state.app_token,
            &body.encrypted_str,
        ]) != params.msg_signature
        {
            tracing::error!("Error checking signature. The request is unsafe.");
            return StatusCode::BAD_REQUEST;
        }

        // Decrypt the message
        let decrypt_result = state.crypto_agent.decrypt(&body.encrypted_str);
        if let Err(e) = &decrypt_result {
            tracing::error!("Error in decrypting: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }

        // Parse the xml document
        let xml_doc = from_str::<ReceivedMsg>(&decrypt_result.unwrap().text);
        if let Err(e) = &xml_doc {
            tracing::error!("Error in xml parsing: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
        received_msg = xml_doc.unwrap();
    }

    // Respond
    tokio::spawn(async move {
        let msg = TextMsg {
            touser: received_msg.from_user_name,
            toparty: "".to_string(),
            totag: "".to_string(),
            msgtype: "text".to_string(),
            agentid: received_msg.agent_id.parse::<usize>().unwrap(),
            safe: 0,
            enable_id_trans: 0,
            enable_duplicate_check: 0,
            duplicate_check_interval: 1800,
            text: TextMsgContent {
                content: received_msg.content,
            },
        };

        // Send the msg
        let state_ro = state.read().unwrap();
        let response = state_ro.wecom_agent.send_text(&msg).await;
        drop(state_ro);
        if let Err(e) = &response {
            tracing::error!("Error sending msg: {}", e);
        }
        let response = response.unwrap();

        // Token out of date?
        if response.error_code() == 40014 {
            tracing::error!("Access token error: {}", response.error_msg());
            // update token
            let mut state_w = state.write().unwrap();
            match state_w.wecom_agent.update_token().await {
                Ok(_) => (),
                Err(e) => {
                    tracing::error!("Update token error: {}", e);
                }
            }
        };
    });

    StatusCode::OK
}

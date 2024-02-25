mod reception;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;

use serde::{Deserialize, Serialize};
use serde_xml_rs::from_str;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;

// 统筹全部逻辑的应用Agent
use reception::Agent;

// 企业微信加解密模块
use wecom_crypto::{generate_signature, CryptoAgent};

// 企业微信API模块
use wecom_agent::{
    message::{MessageBuilder, Text},
    MsgSendResponse, WecomAgent,
};

// Shared state used in all routers
type SharedState = Arc<RwLock<AppState>>;

#[derive(Clone)]
struct AppState {
    app_token: String,
    crypto_agent: CryptoAgent,
    wecom_agent: WecomAgent,
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
    // Try init the wecom agent. Internet connection is required for fetching
    // access token from WeCom server, meaning this may fail.
    let wecom_agent = WecomAgent::new(corp_id, secret).await;
    if let Err(e) = wecom_agent {
        panic!("Initialization failed. {}", e);
    }
    let wecom_agent = wecom_agent.expect("wecom agent should be initialized.");

    // Init a router with this shared state.
    let state = Arc::new(RwLock::new(AppState {
        app_token: String::from(app_token),
        crypto_agent: CryptoAgent::new(b64encoded_aes_key),
        wecom_agent,
        app_agent: Agent::new(oai_endpoint, oai_key),
    }));
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
    // Acquire the lock
    let state = state.read().await;

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

    // Is this request safe?
    {
        let state = state.read().await;
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
    }

    // Respond
    tokio::spawn(async move {
        process_user_msg(state, body).await;
    });

    StatusCode::OK
}

async fn process_user_msg(state: Arc<RwLock<AppState>>, body: RequestBody) {
    // 获取用户的消息
    let received_msg: ReceivedMsg;
    {
        // Acquire a read lock
        let state = state.read().await;

        // Decrypt the user message
        let decrypt_result = state.crypto_agent.decrypt(&body.encrypted_str);
        if let Err(e) = &decrypt_result {
            tracing::error!("Error in decrypting: {}", e);
            return;
        }

        // Parse the xml document
        let xml_doc = from_str::<ReceivedMsg>(&decrypt_result.unwrap().text);
        if let Err(e) = &xml_doc {
            tracing::error!("Error in xml parsing: {}", e);
            return;
        }
        received_msg = xml_doc.expect("XML document should be valid.");
    }

    // 使用AI处理用户消息
    let reply: String;
    {
        let mut state = state.write().await;
        let ai_reply = state
            .app_agent
            .handle_user_message(&received_msg.from_user_name, &received_msg.content)
            .await;
        if let Err(e) = &ai_reply {
            tracing::error!("Error reply user message: {}", e);
            return;
        }
        reply = ai_reply.expect("User message should be handled");
    }

    // 回复给用户的消息
    let content = Text::new(reply);
    let msg = MessageBuilder::default()
        .to_users(vec![&received_msg.from_user_name])
        .from_agent(received_msg.agent_id.parse::<usize>().unwrap())
        .build(content)
        .expect("Massage should be built");
    let response: Result<MsgSendResponse, Box<dyn std::error::Error + Send + Sync>>;
    {
        let state = state.read().await;
        response = state.wecom_agent.send(msg).await;
    }
    if let Err(e) = &response {
        tracing::error!("Error sending msg: {}", e);
        return;
    }
    let response = response.unwrap();

    // Token out of date?
    if response.error_code() == 40014 {
        tracing::error!("Access token error: {}", response.error_msg());
        let mut state = state.write().await;
        // update token
        match state.wecom_agent.update_token().await {
            Ok(_) => {
                tracing::info!("Access token error: {}", response.error_msg());
            }
            Err(e) => {
                tracing::error!("Update token error: {}", e);
            }
        }
    };
}

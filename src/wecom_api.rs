//! 企业微信Server端API返回结果涉及到的数据结构
use serde::Deserialize;

/// 服务器可用性验证请求涉及到的URL参数
#[derive(Deserialize)]
pub struct UrlVerifyParams {
    pub msg_signature: String,
    pub timestamp: String,
    pub nonce: String,
    pub echostr: String,
}

/// 应用消息接收时涉及到的URL参数
#[derive(Deserialize)]
pub struct AppMessageParams {
    pub msg_signature: String,
    pub nonce: String,
    pub timestamp: String,
}

/// 应用消息接收时的Body结构体
///
/// | 参数        | 说明
/// | ToUserName | 企业微信的CorpID，当为第三方套件回调事件时，CorpID的内容为suiteid
/// | AgentID    | 接收的应用id，可在应用的设置页面获取
/// | Encrypt    | 消息结构体加密后的字符串
///
/// 样例：
// <xml>
//   <ToUserName><![CDATA[toUser]]></ToUserName>
//   <AgentID><![CDATA[toAgentID]]></AgentID>
//   <Encrypt><![CDATA[msg_encrypt]]></Encrypt>
// </xml>
#[derive(Debug, Deserialize, PartialEq)]
pub struct AppMessageRequestBody {
    #[serde(rename = "ToUserName")]
    pub to_user_name: String,
    #[serde(rename = "AgentID")]
    pub agent_id: String,
    #[serde(rename = "Encrypt")]
    pub encrypted_str: String,
}

/// 应用消息接收后具体内容结构体
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
pub struct AppMessageContent {
    #[serde(rename = "ToUserName")]
    pub to_user_name: String,
    #[serde(rename = "FromUserName")]
    pub from_user_name: String,
    #[serde(rename = "CreateTime")]
    pub create_time: u64,
    #[serde(rename = "MsgType")]
    pub msg_type: String,
    #[serde(rename = "Content")]
    pub content: String,
    #[serde(rename = "MsgId")]
    pub msg_id: String,
    #[serde(rename = "AgentID")]
    pub agent_id: String,
}

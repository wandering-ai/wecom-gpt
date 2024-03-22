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

/// 回调消息的URL参数
/// | 参数          | 类型     | 说明
/// | msg_signature | String  | 企业微信加密签名，msg_signature结合了企业填写的token、请求中的timestamp、nonce参数、加密的消息体
/// | timestamp     | Integer | 时间戳。与nonce结合使用，用于防止请求重放攻击。
/// | nonce         | String  | 随机数。与timestamp结合使用，用于防止请求重放攻击。
#[derive(Deserialize)]
pub struct CallbackParams {
    pub msg_signature: String,
    pub nonce: String,
    pub timestamp: String,
}

/// 回调消息的Body结构体
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
pub struct CallbackRequestBody {
    #[serde(rename = "ToUserName")]
    pub to_user_name: String,
    #[serde(rename = "AgentID")]
    pub agent_id: String,
    #[serde(rename = "Encrypt")]
    pub encrypted_str: String,
}

/// 应用消息接收后具体内容结构体
/// | 参数          | 说明
/// | ToUserName    | 企业微信CorpID
/// | FromUserName  | 成员UserID
/// | CreateTime    | 消息创建时间（整型）
/// | MsgType       | 消息类型，此时固定为：text
/// | Content       | 文本消息内容
/// | MsgI          | 消息id，64位整型
/// | AgentID       | 企业应用的id，整型。可在应用的设置页面查看
///
/// 示例
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

/// 企业微信通讯录更新事件回调结构体
/// | 参数            | 说明
/// | UserID         | 成员UserID
/// | Department     | 成员部门列表，仅返回该应用有查看权限的部门id
///
/// 示例
/// <xml>
///   <UserID><![CDATA[zhangsan]]></UserID>
///   <Department><![CDATA[1,2,3]]></Department>
/// </xml>
#[derive(Debug, Deserialize, PartialEq)]
pub struct ContactEventContent {
    #[serde(rename = "UserID")]
    pub user_id: String,
}

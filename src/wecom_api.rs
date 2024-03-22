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
/// | ToUserName     | 企业微信CorpID
/// | FromUserName   | 此事件该值固定为sys，表示该消息由系统生成
/// | CreateTime     | 消息创建时间 （整型）
/// | MsgType        | 消息的类型，此时固定为event
/// | Event          | 事件的类型，此时固定为change_contact
/// | ChangeType     | 此时固定为create_user
/// | UserID         | 成员UserID
/// | Name           | 成员名称;代开发自建应用需要管理员授权才返回
/// | Department     | 成员部门列表，仅返回该应用有查看权限的部门id
/// | MainDepartment | 主部门
/// | IsLeaderInDept | 表示所在部门是否为部门负责人，0-否，1-是，顺序与Department字段的部门逐一对应。第三方通讯录应用或者授权了“组织架构信息-应用可获取企业的部门组织架构信息-部门负责人”权限的第三方应用和代开发应用可获取；对于非第三方创建的成员，第三方通讯录应用不可获取；上游企业不可获取下游企业成员该字段
/// | DirectLeader   | 直属上级UserID，最多1个。第三方通讯录应用或者授权了“组织架构信息-应用可获取可见范围内成员组织架构信息-直属上级”权限的第三方应用和代开发应用可获取；对于非第三方创建的成员，第三方通讯录应用不可获取；上游企业不可获取下游企业成员该字段
/// | Mobile         | 手机号码，代开发自建应用需要管理员授权且成员oauth2授权获取；第三方仅通讯录应用可获取；对于非第三方创建的成员，第三方通讯录应用也不可获取；上游企业不可获取下游企业成员该字段
/// | Position       | 职位信息。长度为0~64个字节;代开发自建应用需要管理员授权才返回。上游共享的应用不返回该字段
/// | Gender         | 性别。0表示未定义，1表示男性，2表示女性。代开发自建应用需要管理员授权且成员oauth2授权获取；第三方仅通讯录应用可获取；对于非第三方创建的成员，第三方通讯录应用也不可获取；上游企业不可获取下游企业成员该字段。注：不可获取指返回值0
/// | Email          | 邮箱，代开发自建应用需要管理员授权且成员oauth2授权获取；第三方仅通讯录应用可获取；对于非第三方创建的成员，第三方通讯录应用也不可获取；上游企业不可获取下游企业成员该字段
/// | BizMail        | 企业邮箱，代开发自建应用需要管理员授权且成员oauth2授权获取；第三方仅通讯录应用可获取；对于非第三方创建的成员，第三方通讯录应用也不可获取；上游企业不可获取下游企业成员该字段
/// | Status         | 激活状态：1=已激活 2=已禁用 4=未激活 已激活代表已激活企业微信或已关注微信插件（原企业号）5=成员退出
/// | Avatar         | 头像url。 注：如果要获取小图将url最后的”/0”改成”/100”即可。代开发自建应用需要管理员授权且成员oauth2授权获取；第三方仅通讯录应用可获取；对于非第三方创建的成员，第三方通讯录应用也不可获取；上游企业不可获取下游企业成员该字段
/// | Alias          | 成员别名。上游共享的应用不返回该字段
/// | Telephone      | 座机;代开发自建应用需要管理员授权才返回。上游共享的应用不返回该字段
/// | Address        | 地址。代开发自建应用需要管理员授权且成员oauth2授权获取；第三方仅通讯录应用可获取；对于非第三方创建的成员，第三方通讯录应用也不可获取；上游企业不可获取下游企业成员该字段
/// | ExtAttr        | 扩展属性;代开发自建应用需要管理员授权才返回。上游共享的应用不返回该字段
/// | Type           | 扩展属性类型: 0-本文 1-网页
/// | Text           | 文本属性类型，扩展属性类型为0时填写
/// | Value          | 文本属性内容
/// | Web            | 网页类型属性，扩展属性类型为1时填写
/// | Title          | 网页的展示标题
/// | Url            | 网页的url
///
/// 示例
/// <xml>
///   <ToUserName><![CDATA[toUser]]></ToUserName>
///   <FromUserName><![CDATA[sys]]></FromUserName>
///   <CreateTime>1403610513</CreateTime>
///   <MsgType><![CDATA[event]]></MsgType>
///   <Event><![CDATA[change_contact]]></Event>
///   <ChangeType>create_user</ChangeType>
///   <UserID><![CDATA[zhangsan]]></UserID>
///   <Name><![CDATA[张三]]></Name>
///   <Department><![CDATA[1,2,3]]></Department>
///   <MainDepartment>1</MainDepartment>
///   <IsLeaderInDept><![CDATA[1,0,0]]></IsLeaderInDept>
///   <DirectLeader><![CDATA[lisi,wangwu]]></DirectLeader>
///   <Position><![CDATA[产品经理]]></Position>
///   <Mobile>13800000000</Mobile>
///   <Gender>1</Gender>
///   <Email><![CDATA[zhangsan@gzdev.com]]></Email>
///   <BizMail><![CDATA[zhangsan@qyycs2.wecom.work]]></BizMail>
///   <Status>1</Status>
///   <Avatar><![CDATA[http://wx.qlogo.cn/mmopen/ajNVdqHZLLA3WJ6DSZUfiakYe37PKnQhBIeOQBO4czqrnZDS79FH5Wm5m4X69TBicnHFlhiafvDwklOpZeXYQQ2icg/0]]></Avatar>
///   <Alias><![CDATA[zhangsan]]></Alias>
///   <Telephone><![CDATA[020-123456]]></Telephone>
///   <Address><![CDATA[广州市]]></Address>
///   <ExtAttr>
///     <Item>
///     <Name><![CDATA[爱好]]></Name>
///     <Type>0</Type>
///     <Text>
///       <Value><![CDATA[旅游]]></Value>
///     </Text>
///     </Item>
///     <Item>
///     <Name><![CDATA[卡号]]></Name>
///     <Type>1</Type>
///     <Web>
///       <Title><![CDATA[企业微信]]></Title>
///       <Url><![CDATA[https://work.weixin.qq.com]]></Url>
///     </Web>
///     </Item>
///   </ExtAttr>
/// </xml>
#[derive(Debug, Deserialize, PartialEq)]
pub struct ContactEventContent {
    #[serde(rename = "UserID")]
    pub user_id: String,
    #[serde(rename = "ChangeType")]
    pub event: String,
}

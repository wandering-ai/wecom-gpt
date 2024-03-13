# wechat-gpt
大语言模型的企业微信接入。借助企业微信插件可以实现微信接入。

## 使用方法
```rust
use wecom_gpt::app;

#[tokio::main]
async fn main() {
    let service = app(
        app_token,
        b64_encoded_aes_key,
        corp_id,
        corp_secret,
        azure_openai_endpoint,
        azure_openai_api_key,
        database_url,
    );
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8088").await.unwrap();
    axum::serve(listener, service).await.unwrap();
}
```
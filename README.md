# wechat-gpt
大语言模型的企业微信接入。借助企业微信插件可以实现微信接入。

## 使用方法
```rust
use wecom_gpt::app;

#[tokio::main]
async fn main() {
    // Init the service
    let service = app("/etc/zoo/zoo.cfg"));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8088").await.unwrap();
    axum::serve(listener, service).await.unwrap();
}
```
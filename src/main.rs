use wechat_gpt::app;

#[tokio::main]
async fn main() {
    // Setup tracing
    tracing_subscriber::fmt().compact().init();

    // run our app with hyper, listening globally on port 8088
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8088").await.unwrap();
    axum::serve(listener, app()).await.unwrap();
}

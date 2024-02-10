use wechat_gpt::app;

#[tokio::main]
async fn main() {
    // Setup tracing
    tracing_subscriber::fmt().compact().init();

    // Init the service
    let app_token = "HWmSYaJCKJFVn9YvbdVEmiYl";
    let encoding_aes_key = "cGCVnNJRgRu6wDgo7gxG2diBovGnRQq1Tqy4Rm4V4qF";
    let service = app(app_token, encoding_aes_key);

    tracing::info!("Listening on port 8088..");

    // Run our app with hyper, listening globally on port 8088
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8088").await.unwrap();
    axum::serve(listener, service).await.unwrap();
}

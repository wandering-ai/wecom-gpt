use serde::Deserialize;
use wecom_gpt::app;

#[derive(Deserialize, Debug)]
struct Configuration {
    app_token: String,
    b64_encoded_aes_key: String,
}

#[tokio::main]
async fn main() {
    // Setup tracing
    tracing_subscriber::fmt().compact().init();

    // Read in configuration from OS env.
    let c: Configuration = envy::from_env::<Configuration>()
        .expect("Please provide APP_TOKEN and B64_ENCODED_AES_KEY env vars");

    // Init the service
    let service = app(&c.app_token, &c.b64_encoded_aes_key);

    tracing::info!("Listening on port 8088..");

    // Run our app with hyper, listening globally on port 8088.
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8088").await.unwrap();
    axum::serve(listener, service).await.unwrap();
}

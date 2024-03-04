use serde::Deserialize;
use tracing_subscriber::EnvFilter;
use wecom_gpt::app;

#[derive(Deserialize, Debug)]
struct Configuration {
    app_token: String,
    b64_encoded_aes_key: String,
    corp_id: String,
    corp_secret: String,
    azure_openai_endpoint: String,
    azure_openai_api_key: String,
}

#[tokio::main]
async fn main() {
    // Setup tracing
    tracing_subscriber::fmt()
        .compact()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Read in configuration from OS env.
    let c: Configuration =
        envy::from_env::<Configuration>().expect("Please provide all required env vars");

    // Init the service
    let service = app(
        &c.app_token,
        &c.b64_encoded_aes_key,
        &c.corp_id,
        &c.corp_secret,
        &c.azure_openai_endpoint,
        &c.azure_openai_api_key,
        "duck.db",
    );

    tracing::info!("Listening on port 8088..");

    // Run our app with hyper, listening globally on port 8088.
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8088").await.unwrap();
    axum::serve(listener, service).await.unwrap();
}

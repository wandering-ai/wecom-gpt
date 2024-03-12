use serde::Deserialize;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use wecom_gpt::app;

// Embed the app version
const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[derive(Deserialize, Debug)]
struct Configuration {
    app_token: String,
    b64_encoded_aes_key: String,
    corp_id: String,
    corp_secret: String,
    azure_openai_endpoint: String,
    azure_openai_api_key: String,
    database_url: String,
}

#[tokio::main]
async fn main() {
    // Setup tracing
    tracing_subscriber::fmt()
        .compact()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();
    tracing::info!("Version: {VERSION}");

    // Read in configuration from OS env.
    let c: Configuration = match envy::from_env::<Configuration>() {
        Err(e) => panic!("运行参数不完整：{e}"),
        Ok(c) => c,
    };

    // Init the service
    let service = app(
        &c.app_token,
        &c.b64_encoded_aes_key,
        &c.corp_id,
        &c.corp_secret,
        &c.azure_openai_endpoint,
        &c.azure_openai_api_key,
        &c.database_url,
    );

    tracing::info!("Listening on port 8088..");

    // Run our app with hyper, listening globally on port 8088.
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8088").await.unwrap();
    axum::serve(listener, service).await.unwrap();
}

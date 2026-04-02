use eyre::{Result};
use rusty_d20::run;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    dotenv::dotenv().ok();

    let model = env::var("AI_MODEL")
        .unwrap_or_else(|_| "llama3.2:1b".to_owned());

    let api_key =
    env::var("API_KEY").unwrap_or_else(|_| {
    eprintln!("API Key not found in the environment");
    String::new()
    });


    let base_url = env::var("BASE_URL").unwrap_or_else(|_|
    "http://localhost:11434/v1".to_owned());

    run(model, api_key, base_url).await?;
    Ok(())
}

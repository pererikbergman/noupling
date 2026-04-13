mod routes;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing::info!("Starting API application...");
    core_logic::initialize()?;

    println!("Application is running.");
    Ok(())
}

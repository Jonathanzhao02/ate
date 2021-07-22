#[cfg(not(target_arch = "wasm32"))]
use log::{info};
#[cfg(not(target_arch = "wasm32"))]
use serde::{Serialize, Deserialize};
#[cfg(not(target_arch = "wasm32"))]
use ate::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Serialize, Deserialize, Clone)]
struct MyTestObject
{
    firstname: String,
    lastname: String,
    data: [u128; 32],
    lines: Vec<String>,
}

#[cfg(target_arch = "wasm32")]
fn main() {
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<(), AteError> {
    env_logger::init();

    // The default configuration will store the redo log locally in the temporary folder
    let mut cfg_ate = ConfAte::default();
    cfg_ate.configured_for(ConfiguredFor::BestPerformance);
    let builder = ChainBuilder::new(&cfg_ate).await.build();

    {
        // We create a chain with a specific key (this is used for the file name it creates)
        let chain = builder.open_local(&ChainKey::from("stress")).await?;
        
        // Prepare
        let session = AteSession::new(&cfg_ate);

        let mut test_obj = MyTestObject {
            firstname: "Joe".to_string(),
            lastname: "Blogs".to_string(),
            data: [123 as u128; 32],
            lines: Vec::new(),
        };
        for n in 0..10 {
            test_obj.lines.push(format!("test {}", n));
        }

        // Do a whole let of work
        info!("stress::running");
        for _ in 0..200 {
            let mut dio = chain.dio(&session).await;
            for _ in 0..500 {
                dio.store(test_obj.clone())?;
            }
            dio.commit().await?;
        }
        info!("stress::finished");
    }

    {
        // We create a chain with a specific key (this is used for the file name it creates)
        let chain = builder.open_local(&ChainKey::from("stress")).await?;

        // Destroy the chain
        chain.single().await.destroy().await.unwrap();
    }
    
    Ok(())
}
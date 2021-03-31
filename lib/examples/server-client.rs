#![allow(unused_imports)]
use log::{info, warn, debug, error};
use serde::{Serialize, Deserialize};
use ate::prelude::*;

#[tokio::main]
async fn main() -> Result<(), AteError>
{
    env_logger::init();

    // Create the server and listen on port 5000
    let mut cfg_mesh = ConfMesh::default();
    let cfg_ate = ConfAte::default();
    let addr = MeshAddress::new(IpAddr::from_str("127.0.0.1").unwrap(), 5000);
    let mut cluster = ConfCluster::default();
    cluster.roots.push(addr.clone());
    cfg_mesh.clusters.push(cluster);
    cfg_mesh.force_listen = Some(addr);
    let server = create_mesh(&cfg_ate, &cfg_mesh).await;

    info!("write some data to the server");    

    let key = {
        let registry = Registry::new(&cfg_ate).await;
        let chain = registry.chain(&url::Url::from_str("tcp://localhost:5000/test-chain").unwrap()).await?;
        let session = AteSession::default();
        let mut dio = chain.dio_ext(&session, TransactionScope::Full).await;
        let dao = dio.store("my test string".to_string())?;
        dio.commit().await?;
        dao.key().clone()
    };

    info!("read it back again on the server");

    let chain = server.open(ChainKey::from("test-chain")).await.unwrap();
    chain.sync().await?;
    let session = AteSession::default();
    let mut dio = chain.dio_ext(&session, TransactionScope::Full).await;
    let dao = dio.load::<String>(&key).await?;

    assert_eq!(*dao, "my test string".to_string());
    Ok(())
}
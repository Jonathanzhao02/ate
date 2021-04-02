#[allow(unused_imports)]
use serde::{Serialize, Deserialize};
use ate::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
enum BallSound
{
    Ping,
    Pong
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Table
{
    ball: DaoVec<BallSound>
}

#[tokio::main]
async fn main() -> Result<(), AteError>
{
    // Create the server and listen on port 5001
    let mut cfg_mesh = ConfMesh::default();
    let cfg_ate = ConfAte::default();
    let addr = MeshAddress::new(IpAddr::from_str("127.0.0.1").unwrap(), 5001);
    let mut cluster = ConfCluster::default();
    cluster.roots.push(addr.clone());
    cfg_mesh.clusters.push(cluster);
    cfg_mesh.force_listen = Some(addr);
    let _ = create_mesh(&cfg_ate,&cfg_mesh).await;

    // Connect to the server from a client
    cfg_mesh.force_listen = None;
    cfg_mesh.force_client_only = true;
    let client_a = create_mesh(&cfg_ate, &cfg_mesh).await;
    let client_b = create_mesh(&cfg_ate, &cfg_mesh).await;

    // Create a session
    let session = AteSession::default();

    // Setup a BUS that we will listen on
    let chain_a = client_a.open(ChainKey::from("ping-pong-table")).await.unwrap();
    let (mut bus, key) = {
        let mut dio = chain_a.dio_ext(&session, TransactionScope::Full).await;
        let dao = dio.store(Table {
            ball: DaoVec::new(),
        })?;
        dio.commit().await?;

        // Now attach a BUS that will simple write to the console
        (
            dao.bus(&chain_a, dao.ball),
            dao.key().clone(),
        )
    };

    {
        // Write a ping... twice
        let chain_b = client_b.open(ChainKey::from("ping-pong-table")).await.unwrap();
        chain_b.sync().await?;
        let mut dio = chain_b.dio_ext(&session, TransactionScope::Full).await;
        let mut dao = dio.load::<Table>(&key).await?;
        dao.push(&mut dio, dao.ball, BallSound::Ping)?;
        dao.push(&mut dio, dao.ball, BallSound::Ping)?;
        dao.commit(&mut dio)?;
        dio.commit().await?;
    }

    // Process any events that were received on the BUS
    {   
        let mut dio = chain_a.dio_ext(&session, TransactionScope::Full).await;

        // (this is an exactly once queue)
        let mut ret = bus.process(&mut dio).await?;
        println!("{:?}", ret);
        ret.commit(&mut dio)?;
        dio.commit().await?;

        // (this is a broadcast event to all current subscribers)
        let ret = bus.recv(&session).await?;
        println!("{:?}", ret);
    }

    Ok(())
}
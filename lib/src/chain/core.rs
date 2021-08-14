#[allow(unused_imports)]
use tracing::{info, warn, debug, error, trace, instrument, span, Level};
use tracing_futures::Instrument;
use parking_lot::Mutex as StdMutex;

use crate::error::*;

use crate::comms::NodeId;
use crate::comms::Metrics;
use crate::comms::Throttle;
use crate::transaction::*;

use std::sync::{Arc};
use tokio::sync::RwLock;
use tokio::sync::broadcast;
use parking_lot::RwLock as StdRwLock;

use crate::single::*;
use crate::multi::*;
use crate::pipe::*;
use crate::meta::*;
use crate::spec::*;
use crate::conf::ConfAte;
use crate::mesh::BackupMode;
use crate::redo::RedoLog;
use crate::time::TimeKeeper;
use crate::transaction::TransactionScope;
use crate::trust::ChainKey;
use crate::trust::ChainHeader;
use crate::engine::*;
use crate::conf::MeshAddress;
use crate::prelude::PrimaryKey;

use super::*;

/// Represents the main API to access a specific chain-of-trust
///
/// This object must stay within scope for the duration of its
/// use which has been optimized for infrequent initialization as
/// creating this object will reload the entire chain's metadata
/// into memory.
///
/// The actual data of the chain is stored locally on disk thus
/// huge chains can be stored here however very random access on
/// large chains will result in random access IO on the disk.
///
/// Chains also allow subscribe/publish models to be applied to
/// particular vectors (see the examples for details)
///
#[derive(Clone)]
pub struct Chain
{
    pub(crate) key: ChainKey,
    pub(crate) node_id: NodeId,
    pub(crate) cfg_ate: ConfAte,
    pub(crate) remote_addr: Option<MeshAddress>,
    pub(crate) default_format: MessageFormat,
    pub(crate) inside_sync: Arc<StdRwLock<ChainProtectedSync>>,
    pub(crate) inside_async: Arc<RwLock<ChainProtectedAsync>>,
    pub(crate) pipe: Arc<Box<dyn EventPipe>>,
    pub(crate) time: Arc<TimeKeeper>,
    pub(crate) exit: broadcast::Sender<()>,
    pub(crate) decache: broadcast::Sender<Vec<PrimaryKey>>,
    pub(crate) metrics: Arc<StdMutex<Metrics>>,
    pub(crate) throttle: Arc<StdMutex<Throttle>>,
}

impl<'a> Chain
{    
    pub(crate) fn proxy(&mut self, mut proxy: Box<dyn EventPipe>) {
        proxy.set_next(Arc::clone(&self.pipe));
        let proxy = Arc::new(proxy);
        let _ = std::mem::replace(&mut self.pipe, proxy);
    }

    pub fn key(&'a self) -> &'a ChainKey {
        &self.key
    }

    pub fn remote_addr(&'a self) -> Option<&'a MeshAddress> {
        self.remote_addr.as_ref()
    }

    pub async fn single(&'a self) -> ChainSingleUser<'a> {
        TaskEngine::run_until(self.__single()).await
    }

    async fn __single(&'a self) -> ChainSingleUser<'a> {
        ChainSingleUser::new(self).await
    }

    pub async fn multi(&'a self) -> ChainMultiUser {
        TaskEngine::run_until(self.__multi()).await
    }

    async fn __multi(&'a self) -> ChainMultiUser {
        ChainMultiUser::new(self).await
    }

    pub async fn name(&'a self) -> String {
        TaskEngine::run_until(self.__name()).await
    }

    async fn __name(&'a self) -> String {
        self.single().await.name()
    }

    pub fn default_format(&'a self) -> MessageFormat {
        self.default_format.clone()
    }

    pub async fn count(&'a self) -> usize {
        TaskEngine::run_until(self.__count()).await
    }

    async fn __count(&'a self) -> usize {
        self.inside_async.read().await.chain.redo.count()
    }

    async fn run_async<F>(&self, future: F) -> F::Output
    where F: std::future::Future,
    {
        let key_str = self.key().to_string();
        TaskEngine::run_until(future
            .instrument(span!(Level::DEBUG, "dio", key=key_str.as_str()))
        ).await
    }

    pub async fn flush(&'a self) -> Result<(), tokio::io::Error> {
        self.run_async(self.__flush()).await
    }

    async fn __flush(&'a self) -> Result<(), tokio::io::Error> {
        Ok(
            self.inside_async.write().await.chain.flush().await?
        )
    }

    pub async fn sync(&'a self) -> Result<(), CommitError>
    {
        self.run_async(self.__sync()).await
    }

    async fn __sync(&'a self) -> Result<(), CommitError>
    {
        // Create the transaction
        let trans = Transaction {
            scope: TransactionScope::Full,
            transmit: true,
            events: Vec::new(),
            conversation: None,
        };

        // Feed the transaction into the chain
        let pipe = self.pipe.clone();
        pipe.feed(ChainWork {
            trans
        }).await?;

        // Success!
        Ok(())
    }

    pub(crate) async fn get_pending_uploads(&self) -> Vec<MetaDelayedUpload>
    {
        let guard = self.inside_async.read().await;
        guard.chain.timeline.pointers.get_pending_uploads()
    }

    pub(crate) async fn shutdown(&self) -> Result<(), tokio::io::Error>
    {
        self.run_async(self.__shutdown()).await
    }

    pub fn metrics(&'a self) -> parking_lot::MutexGuard<'_, Metrics>
    {
        self.metrics.lock()
    }

    pub fn throttl(&'a self) -> parking_lot::MutexGuard<'_, Throttle>
    {
        self.throttle.lock()
    }

    async fn __shutdown(&self) -> Result<(), tokio::io::Error>
    {
        let include_active_files = match self.cfg_ate.backup_mode {
            BackupMode::None => { return Ok(()); },
            BackupMode::Restore => { return Ok(()); },
            BackupMode::Rotating => false,
            BackupMode::Full => true,
        };
        
        let mut single = self.single().await;
        if single.inside_async.is_shutdown == false {
            single.inside_async.is_shutdown = true;
            
            single.inside_async.chain.redo.backup(include_active_files)?.await?;
        }
        Ok(())
    }
}

impl Drop
for Chain
{
    fn drop(&mut self)
    {
        trace!("drop {}", self.key.to_string());
        let _ = self.exit.send(());
    }
}

impl RedoLog
{
    pub(crate) fn read_chain_header(&self) -> Result<ChainHeader, SerializationError>
    {
        let header_bytes = self.header(u32::MAX);
        Ok
        (
            if header_bytes.len() > 0 {
                SerializationFormat::Json.deserialize(&header_bytes[..])?
            } else {
                ChainHeader::default()
            }
        )
    }
}
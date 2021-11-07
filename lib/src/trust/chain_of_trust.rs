#[allow(unused_imports)]
use tracing::{info, warn, debug, error, trace, instrument, span, Level};
use std::sync::Mutex as StdMutex;
use std::sync::Arc;

use crate::comms::Metrics;
use crate::meta::*;
use crate::error::*;
use crate::header::*;
use crate::event::*;
use crate::index::*;
use crate::redo::*;

use super::*;

pub(crate) struct ChainOfTrust
{
    pub(crate) debug_id: u64,
    pub(crate) key: ChainKey,
    pub(crate) timeline: ChainTimeline,
    pub(crate) redo: RedoLog,
    pub metrics: Arc<StdMutex<Metrics>>,
}

impl std::fmt::Debug
for ChainOfTrust
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "chain-of-trust(key={},debug_id={})", self.key, self.debug_id)
    }
}

impl<'a> ChainOfTrust
{
    pub(crate) async fn load(&self, leaf: EventLeaf) -> Result<LoadResult, LoadError>
    {
        #[cfg(feature = "enable_verbose")]
        debug!("loading: {}", leaf.record);
        
        let data = self.redo.load(leaf.record.clone()).await?;
        Ok(LoadResult {
            lookup: data.lookup,
            header: data.header,
            data: data.data,
            leaf: leaf,
        })
    }

    pub(crate) async fn load_many(&self, leafs: Vec<EventLeaf>) -> Result<Vec<LoadResult>, LoadError>
    {
        let mut ret = Vec::new();

        let mut futures = Vec::new();
        for leaf in leafs.into_iter() {
            let data = self.redo.load(leaf.record.clone());
            futures.push((data, leaf));
        }

        for (join, leaf) in futures.into_iter() {
            let data = join.await?;
            ret.push(LoadResult {
                lookup: data.lookup,
                header: data.header,
                data: data.data,
                leaf,
            });
        }

        Ok(ret)
    }

    pub(crate) fn lookup_primary(&self, key: &PrimaryKey) -> Option<EventLeaf>
    {
        self.timeline.lookup_primary(key)
    }

    pub(crate) fn lookup_parent(&self, key: &PrimaryKey) -> Option<MetaParent> {
        self.timeline.lookup_parent(key)
    }

    pub(crate) fn lookup_secondary(&self, key: &MetaCollection) -> Option<Vec<EventLeaf>>
    {
        self.timeline.lookup_secondary(key)
    }

    pub(crate) fn lookup_secondary_raw(&self, key: &MetaCollection) -> Option<Vec<PrimaryKey>>
    {
        self.timeline.lookup_secondary_raw(key)
    }

    pub(crate) fn invalidate_caches(&mut self) {
        self.timeline.invalidate_caches();
    }

    pub(crate) async fn flush(&mut self) -> Result<(), tokio::io::Error> {
        self.redo.flush().await
    }

    pub(crate) async fn destroy(&mut self) -> Result<(), tokio::io::Error> {
        self.invalidate_caches();
        self.redo.destroy()
    }

    pub(crate) fn name(&self) -> String {
        self.key.name.clone()
    }

    pub(crate) fn add_history(&mut self, header: EventHeader)
    {
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.chain_size += header.raw.meta_bytes.len() as u64;
            metrics.chain_size += header.raw.data_size as u64;
        }
        self.timeline.add_history(header)
    }
}
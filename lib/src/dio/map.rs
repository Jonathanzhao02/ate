#![allow(unused_imports)]
use tracing::{info, warn, debug, error, trace, instrument, span, Level};
use tracing_futures::Instrument;
use error_chain::bail;
use std::marker::PhantomData;
use std::sync::{Arc, Weak};
use fxhash::FxHashMap;

use serde::*;
use serde::de::*;
use super::dio::DioWeak;
use super::dio_mut::DioMutWeak;
use crate::dio::*;
use crate::dio::dao::*;
use super::vec::DaoVecState;
use crate::error::*;
use std::collections::VecDeque;
use crate::prelude::*;

#[derive(Serialize, Deserialize)]
pub struct DaoMap<K, V>
where K: Eq + std::hash::Hash
{
    pub(super) lookup: FxHashMap<K, PrimaryKey>,
    pub(super) vec_id: u64,
    #[serde(skip)]
    pub(super) state: DaoMapState,
    #[serde(skip)]
    dio: DioWeak,
    #[serde(skip)]
    dio_mut: DioMutWeak,
    #[serde(skip)]
    _phantom1: PhantomData<V>,
}

pub(super) enum DaoMapState
{
    Unsaved,
    Saved(PrimaryKey)
}

impl Default
for DaoMapState
{
    fn default() -> Self
    {
        match PrimaryKey::current_get() {
            Some(a) => DaoMapState::Saved(a),
            None => DaoMapState::Unsaved
        }
    }
}

impl Clone
for DaoMapState
{
    fn clone(&self) -> Self
    {
        match self {
            Self::Unsaved => Self::default(),
            Self::Saved(a) => Self::Saved(a.clone())
        }
    }
}  

impl<K, V> std::fmt::Debug
for DaoMap<K, V>
where K: Eq + std::hash::Hash,
      V: std::fmt::Debug
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let key_type_name = std::any::type_name::<K>();
        let value_type_name = std::any::type_name::<V>();
        write!(f, "dao-map(vec_id={}, key-type={}, value-type={}", self.vec_id, key_type_name, value_type_name)
    }
}

impl<K, V> Default
for DaoMap<K, V>
where K: Eq + std::hash::Hash
{
    fn default() -> Self {
        DaoMap::new()
    }
}

impl<K, V> Clone
for DaoMap<K, V>
where K: Clone + Eq + std::hash::Hash
{
    fn clone(&self) -> DaoMap<K, V>
    {
        DaoMap {
            lookup: self.lookup.clone(),
            state: self.state.clone(),
            vec_id: self.vec_id,
            dio: self.dio.clone(),
            dio_mut: self.dio_mut.clone(),
            _phantom1: PhantomData,
        }
    }
}

impl<K, V> DaoMap<K, V>
where K: Eq + std::hash::Hash
{
    pub fn new() -> DaoMap<K, V> {
        DaoMap {
            lookup: FxHashMap::default(),
            state: DaoMapState::Unsaved,
            dio: DioWeak::Uninitialized,
            dio_mut: DioMutWeak::Uninitialized,
            vec_id: fastrand::u64(..),
            _phantom1: PhantomData,
        }
    }
}

impl<K, V> DaoMap<K, V>
where K: Eq + std::hash::Hash
{
    pub fn new_orphaned(dio: &Arc<Dio>, parent: PrimaryKey, vec_id: u64) -> DaoMap<K, V> {
        DaoMap {
            lookup: FxHashMap::default(),
            state: DaoMapState::Saved(parent),
            dio: DioWeak::from(dio),
            dio_mut: DioMutWeak::Uninitialized,
            vec_id: vec_id,
            _phantom1: PhantomData,
        }
    }

    pub fn new_orphaned_mut(dio: &Arc<DioMut>, parent: PrimaryKey, vec_id: u64) -> DaoMap<K, V> {
        DaoMap {
            lookup: FxHashMap::default(),
            state: DaoMapState::Saved(parent),
            dio: DioWeak::from(&dio.dio),
            dio_mut: DioMutWeak::from(dio),
            vec_id: vec_id,
            _phantom1: PhantomData,
        }
    }

    pub fn dio(&self) -> Option<Arc<Dio>> {
        match &self.dio {
            DioWeak::Uninitialized => None,
            DioWeak::Weak(a) => Weak::upgrade(a)
        }
    }

    pub fn dio_mut(&self) -> Option<Arc<DioMut>> {
        match &self.dio_mut {
            DioMutWeak::Uninitialized => None,
            DioMutWeak::Weak(a) => Weak::upgrade(a)
        }
    }

    pub fn as_vec(&self) -> DaoVec<V> {
        DaoVec {
            vec_id: self.vec_id,
            state: match &self.state {
                DaoMapState::Saved(a) => DaoVecState::Saved(a.clone()),
                DaoMapState::Unsaved => DaoVecState::Unsaved,
            },
            dio: self.dio.clone(),
            dio_mut: self.dio_mut.clone(),
            _phantom1: PhantomData,  
        }
    }

    pub fn vec_id(&self) -> u64 {
        self.vec_id
    }

    pub async fn len(&self) -> Result<usize, LoadError>
    {
        let len = match &self.state {
            DaoMapState::Unsaved => 0usize,
            DaoMapState::Saved(parent_id) =>
            {
                let dio = match self.dio() {
                    Some(a) => a,
                    None => bail!(LoadErrorKind::WeakDio)
                };
                
                dio.children_keys(parent_id.clone(), self.vec_id).await?.len()
            },
        };
        Ok(len)
    }

    pub async fn iter<'a>(&'a self) -> Result<Iter<'a, K, V>, LoadError>
    where V: Serialize + DeserializeOwned
    {
        self.iter_ext(false, false).await
    }

    pub async fn iter_ext<'a>(&'a self, allow_missing_keys: bool, allow_serialization_error: bool) -> Result<Iter<'a, K, V>, LoadError>
    where V: Serialize + DeserializeOwned
    {
        let mut reverse = FxHashMap::default();
        for (k, v) in self.lookup.iter() {
            reverse.insert(v, k);
        }

        let children = match &self.state {
            DaoMapState::Unsaved => vec![],
            DaoMapState::Saved(parent_id) =>
            {
                if let Some(dio) = self.dio_mut() {
                    dio.children_ext(parent_id.clone(), self.vec_id, allow_missing_keys, allow_serialization_error).await?
                        .into_iter()
                        .map(|a: DaoMut<V>| a.inner)
                        .collect::<Vec<_>>()
                } else {
                    let dio = match self.dio() {
                        Some(a) => a,
                        None => bail!(LoadErrorKind::WeakDio)
                    };
                    
                    dio.children_ext(parent_id.clone(), self.vec_id, allow_missing_keys, allow_serialization_error).await?
                }
            },
        };

        let pairs = children.into_iter()
            .filter_map(|v| {
                match reverse.get(v.key()) {
                    Some(k) => Some((*k, v)),
                    None => None
                }
            })
            .collect::<Vec<_>>();

        Ok(
            Iter::new(
            pairs                
            )
        )
    }

    pub async fn iter_mut(&mut self) -> Result<IterMut<'_, K, V>, LoadError>
    where V: Serialize + DeserializeOwned
    {
        self.iter_mut_ext(false, false).await
    }

    pub async fn iter_mut_ext<'a>(&'a mut self, allow_missing_keys: bool, allow_serialization_error: bool) -> Result<IterMut<'a, K, V>, LoadError>
    where V: Serialize + DeserializeOwned
    {
        let mut reverse = FxHashMap::default();
        for (k, v) in self.lookup.iter() {
            reverse.insert(v, k);
        }

        let children = match &self.state {
            DaoMapState::Unsaved => vec![],
            DaoMapState::Saved(parent_id) =>
            {
                let dio = match self.dio_mut() {
                    Some(a) => a,
                    None => bail!(LoadErrorKind::WeakDio)
                };
                
                let mut ret = Vec::default();
                for child in dio.children_ext::<V>(parent_id.clone(), self.vec_id, allow_missing_keys, allow_serialization_error).await? {
                    ret.push(child)
                }
                ret
            },
        };

        let pairs = children.into_iter()
            .filter_map(|v| {
                match reverse.get(v.key()) {
                    Some(k) => Some((*k, v)),
                    None => None
                }
            })
            .collect::<Vec<_>>();

        Ok(
            IterMut::new(
            pairs                
            )
        )
    }

    pub async fn insert(&mut self, key: K, value: V) -> Result<(), SerializationError>
    where K: Eq + std::hash::Hash,
          V: Clone + Serialize + DeserializeOwned,
    {
        self.insert_ret(key, value).await?;
        Ok(())
    }

    pub async fn insert_ret(&mut self, key: K, value: V) -> Result<DaoMut<V>, SerializationError>
    where K: Eq + std::hash::Hash,
          V: Clone + Serialize + DeserializeOwned,
    {
        let dio = match self.dio_mut() {
            Some(a) => a,
            None => bail!(SerializationErrorKind::WeakDio)
        };

        let parent_id = match &self.state {
            DaoMapState::Unsaved => { bail!(SerializationErrorKind::SaveParentFirst); },
            DaoMapState::Saved(a) => a.clone(),
        };

        let mut ret = dio.store(value)?;
        ret.attach_ext(parent_id, self.vec_id)?;

        if let Some(old) = self.lookup.insert(key, ret.key().clone()) {
            dio.delete(&old).await?;
        }

        Ok(ret)
    }

    pub async fn get(&mut self, key: &K) -> Result<Option<DaoMut<V>>, LoadError>
    where K: Eq + std::hash::Hash,
          V: Serialize + DeserializeOwned
    {
        let id = match self.lookup.get(key) {
            Some(a) => a,
            None => {
                return Ok(None);
            }
        };

        let dio = match self.dio_mut() {
            Some(a) => a,
            None => bail!(LoadErrorKind::WeakDio)
        };

        if dio.exists(&id).await == false {
            return Ok(None);
        }

        let ret = match dio.load::<V>(&id).await {
            Ok(a) => Some(a),
            Err(LoadError(LoadErrorKind::NotFound(_), _)) => None,
            Err(err) => { bail!(err); }
        };
        Ok(ret)
    }

    pub async fn delete(&mut self, key: &K) -> Result<bool, SerializationError>
    where K: Eq + std::hash::Hash,
          V: Serialize
    {
        let id = match self.lookup.get(key) {
            Some(a) => a,
            None => {
                return Ok(false);
            }
        };

        let dio = match self.dio_mut() {
            Some(a) => a,
            None => bail!(SerializationErrorKind::WeakDio)
        };

        if dio.exists(&id).await == false {
            return Ok(false);
        }

        dio.delete(&id).await?;
        Ok(true)
    }
}

pub struct Iter<'a, K, V>
{
    vec: VecDeque<(&'a K, Dao<V>)>,
}

impl<'a, K, V> Iter<'a, K, V>
{
    pub(super) fn new(vec: Vec<(&'a K, Dao<V>)>) -> Iter<'a, K, V> {
        Iter {
            vec: VecDeque::from(vec),
        }
    }
}

impl<'a, K, V> Iterator
for Iter<'a, K, V>
{
    type Item = (&'a K, Dao<V>);

    fn next(&mut self) -> Option<(&'a K, Dao<V>)> {
        self.vec.pop_front()
    }
}

pub struct IterMut<'a, K, V>
where V: Serialize
{
    vec: VecDeque<(&'a K, DaoMut<V>)>,
}

impl<'a, K, V> IterMut<'a, K, V>
where V: Serialize
{
    pub(super) fn new(vec: Vec<(&'a K, DaoMut<V>)>) -> IterMut<'a, K, V> {
        IterMut {
            vec: VecDeque::from(vec),
        }
    }
}

impl<'a, K, V> Iterator
for IterMut<'a, K, V>
where V: Serialize
{
    type Item = (&'a K, DaoMut<V>);

    fn next(&mut self) -> Option<(&'a K, DaoMut<V>)> {
        self.vec.pop_front()
    }
}
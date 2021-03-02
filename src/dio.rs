use linked_hash_map::LinkedHashMap;
use fxhash::{FxHashMap, FxHashSet};

#[cfg(test)]
use serde::{Deserialize};
use serde::{Serialize, de::DeserializeOwned};
use std::io::Result;
use std::io::Error;
use std::io::ErrorKind;
use bytes::Bytes;
use std::cell::{RefCell};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use super::header::*;
use super::chain::*;
use super::event::*;
use super::meta::*;

#[allow(dead_code)]
type Dio<'a> = DioExt<'a, EmptyMetadata>;
#[allow(dead_code)]
type Dao<D> = DaoExt<EmptyMetadata, D>;

#[derive(Debug, Clone)]
struct Row<M, D>
where M: OtherMetadata,
      D: Serialize + DeserializeOwned + Clone,
{
    pub key: PrimaryKey,
    pub meta: Metadata<M>,
    pub data: D,
}

impl<D, M> Row<M, D>
where M: OtherMetadata,
      D: Serialize + DeserializeOwned + Clone,
{
    #[allow(dead_code)]
    pub fn new(key: PrimaryKey, meta: Metadata<M>, data: D) -> Row<M, D> {
        Row {
            key: key,
            meta: meta,
            data: data,
        }
    }

    pub fn from_event(evt: &Event<M>) -> Result<Row<M, D>> {
        match &evt.body {
            Some(data) => {
                match bincode::deserialize(&data) {
                    std::result::Result::Ok(a) => {
                        Ok(
                            Row {
                                key: evt.header.key.clone(),
                                meta: evt.header.meta.clone(),
                                data: a,
                            }
                        )
                    },
                    std::result::Result::Err(err) => Result::Err(Error::new(ErrorKind::Other, format!("{}", err))),
                }
            }
            None => return Result::Err(Error::new(ErrorKind::NotFound, format!("Found a record but it has no data to load for {}", evt.header.key.as_hex_string()))),
        }
    }

    pub fn as_event(&self) -> std::result::Result<Event<M>, bincode::Error> {
        Ok
        (
            Event {
                header: Header {
                    key: self.key.clone(),
                    meta: self.meta.clone(),
                },
                body: Some(Bytes::from(bincode::serialize(&self.data)?)),
            }
        )
    }
}

#[derive(Debug)]
pub struct DaoExt<M, D>
where M: OtherMetadata,
      D: Serialize + DeserializeOwned + Clone,
{
    dirty: bool,
    row: Row<M, D>,
    state: Rc<RefCell<DioState<M>>>
}

impl<M, D> DaoExt<M, D>
where M: OtherMetadata,
      D: Serialize + DeserializeOwned + Clone,
{
    fn new<>(row: Row<M, D>, state: &Rc<RefCell<DioState<M>>>) -> DaoExt<M, D> {
        DaoExt {
            dirty: false,
            row: row,
            state: Rc::clone(state),
        }
    }

    fn fork(&mut self) -> bool {
        if self.dirty == false {
            let mut state = self.state.borrow_mut();
            if state.lock(&self.row.key) == false {
                eprintln!("Detected concurrent writes on data object ({:?}) - the last one in scope will override the all other changes made", self.row.key);
            }
            self.dirty = true;
        }
        true
    }

    pub fn flush(&mut self) -> std::result::Result<(), bincode::Error> {
        if self.dirty == true
        {            
            let mut state = self.state.borrow_mut();
            state.unlock(&self.row.key);
            
            self.dirty = false;

            let evt = self.row.as_event()?;
            state.dirty(evt);
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn key(&self) -> &PrimaryKey {
        &self.row.key
    }

    #[allow(dead_code)]
    pub fn metadata(&self) -> &Metadata<M> {
        &self.row.meta
    }

    #[allow(dead_code)]
    pub fn metadata_mut(&mut self) -> &mut Metadata<M> {
        self.fork();
        &mut self.row.meta
    }

    #[allow(dead_code)]
    pub fn delete(self) {
        let mut state = self.state.borrow_mut();
        if state.lock(&self.row.key) == false {
            eprintln!("Detected concurrent write while deleting a data object ({:?}) - the delete operation will override everything else", self.row.key);
        }
        let key = self.key().clone();
        state.dirty.remove(&key);
        state.cache.remove(&key);
        state.deleted.insert(key);
    }
}

impl<M, D> Deref for DaoExt<M, D>
where M: OtherMetadata,
      D: Serialize + DeserializeOwned + Clone,
{
    type Target = D;

    fn deref(&self) -> &Self::Target {
        &self.row.data
    }
}

impl<M, D> DerefMut for DaoExt<M, D>
where M: OtherMetadata,
      D: Serialize + DeserializeOwned + Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.fork();
        &mut self.row.data
    }
}

impl<M, D> Drop for DaoExt<M, D>
where M: OtherMetadata,
      D: Serialize + DeserializeOwned + Clone,
{
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

#[derive(Debug)]
struct DioState<M>
where M: OtherMetadata,
{
    dirty: LinkedHashMap<PrimaryKey, Rc<Event<M>>>,
    cache: FxHashMap<PrimaryKey, Rc<Event<M>>>,
    locked: FxHashSet<PrimaryKey>,
    deleted: FxHashSet<PrimaryKey>,
}

impl<M> DioState<M>
where M: OtherMetadata,
{
    fn dirty(&mut self, dao: Event<M>) {
        self.cache.remove(&dao.header.key);
        self.dirty.insert(dao.header.key, Rc::new(dao));
    }

    fn lock(&mut self, key: &PrimaryKey) -> bool {
        self.locked.insert(key.clone())
    }

    fn unlock(&mut self, key: &PrimaryKey) -> bool {
        self.locked.remove(key)
    }

    fn is_locked(&self, key: &PrimaryKey) -> bool {
        self.locked.contains(key)
    }
}

impl<M> DioState<M>
where M: OtherMetadata,
{
    fn new() -> DioState<M> {
        DioState {
            dirty: LinkedHashMap::new(),
            cache: FxHashMap::default(),
            locked: FxHashSet::default(),
            deleted: FxHashSet::default(),
        }
    }
}

pub struct DioExt<'a, M>
where M: OtherMetadata,
{
    accessor: ChainAccessorExt<M>,
    multi: Option<ChainMultiUserExt<'a, M>>,
    state: Rc<RefCell<DioState<M>>>,
}

impl<'a, M> DioExt<'a, M>
where M: OtherMetadata,      
{
    #[allow(dead_code)]
    pub fn store<D>(&mut self, metadata: Metadata<M>, data: D) -> DaoExt<M, D>
    where D: Serialize + DeserializeOwned + Clone,
    {
        let key = PrimaryKey::generate();
        let body = Bytes::from(bincode::serialize(&data).unwrap());
        let evt = Event {
            header: Header {
                key: key.clone(),
                meta: metadata.clone(),
            },
            body: Some(body),
        };
        self.state.borrow_mut().dirty.insert(key.clone(), Rc::new(evt));

        let row = Row {
            key: key,
            meta: metadata,
            data: data,
        };
        DaoExt::new(row, &self.state)
    }

    #[allow(dead_code)]
    pub fn load<D>(&mut self, key: &PrimaryKey) -> Result<DaoExt<M, D>>
    where D: Serialize + DeserializeOwned + Clone,
    {
        let mut state = self.state.borrow_mut();
        if state.is_locked(key) {
            return Result::Err(Error::new(ErrorKind::Other, format!("The record is locked as it has a dirty record already in scope for this call stack {:?}", key)));
        }
        if let Some(dao) = state.dirty.get(key) {
            let row = Row::from_event(dao.deref())?;
            return Ok(DaoExt::new(row, &self.state));
        }
        if let Some(dao) = state.cache.get(key) {
            let row = Row::from_event(dao.deref())?;
            return Ok(DaoExt::new(row, &self.state));
        }
        if state.deleted.contains(key) {
            return Result::Err(Error::new(ErrorKind::NotFound, format!("Record with this key has already been deleted {}", key.as_hex_string())))
        }

        let multi = self.multi.as_ref().ok_or(Error::new(ErrorKind::Other, "Dio is not properly initialized (missing multiuser chain handle)"))?;

        let entry = match multi.search(key) {
            Some(a) => a,
            None => return Result::Err(Error::new(ErrorKind::NotFound, format!("Failed to find a record for {}", key.as_hex_string()))),
        };
        if entry.header.meta.has_tombstone() {
            return Result::Err(Error::new(ErrorKind::NotFound, format!("Found a record but its been tombstoned for {}", key.as_hex_string())));
        }

        let mut evt = multi.load(&entry)?;
        evt.body = match evt.body {
            Some(data) => Some(multi.data_as_overlay(&mut evt.header.meta, data)?),
            None => None,
        };

        let row = Row::from_event(&evt)?;
        state.cache.insert(key.clone(), Rc::new(evt));
        Ok(DaoExt::new(row, &self.state))
    }
}

impl<'a, M> Drop
for DioExt<'a, M>
where M: OtherMetadata,
{
    fn drop(&mut self)
    {
        // If we have dirty records
        let state = self.state.borrow_mut();
        if state.dirty.is_empty() == false || state.deleted.is_empty() == false
        {
            let mut evts = Vec::new();
            {
                // Take the reference to the multi for a limited amount of time then destruct it and release the lock
                let multi = self.multi.take().expect("The multilock was released before the drop call was triggered by the Dio going out of scope.");

                // Convert all the events that we are storing into serialize data
                for (_, dao) in state.dirty.iter() {
                    let mut meta = dao.header.meta.clone();
                    let data = match &dao.body {
                        Some(body) => {
                            match multi.data_as_underlay(&mut meta, body.clone()) {
                                Ok(a) => Some(a),
                                Err(err) => {
                                    debug_assert!(false, err.to_string());
                                    eprintln!("{}", err.to_string());
                                    continue;
                                },
                            }
                        },
                        None => None,
                    };

                    let evt = Event {
                        header: Header {
                            key: dao.header.key,
                            meta: meta,
                        },
                        body: data,
                    };
                    evts.push(evt);
                }
            }

            // Build events that will represent tombstones on all these records (they will be sent after the writes)
            for key in &state.deleted {
                let mut evt = Event {
                    header: Header {
                        key: key.clone(),
                        meta: Metadata::default(),
                    },
                    body: None,
                };
                evt.header.meta.add_tombstone();
                evts.push(evt);
            }

            // Process it in the chain of trust
            let mut single = self.accessor.single();
            single.feed(evts).unwrap();
        }
    }
}

impl<M> Metadata<M>
where M: OtherMetadata
{
    #[allow(dead_code)]
    pub fn add_tombstone(&mut self) {
        if self.has_tombstone() == true { return; }
        self.core.push(CoreMetadata::Tombstone);
    }
}

pub trait DioFactoryExt<'a, M>
where M: OtherMetadata,
{
    fn dio(&'a mut self) -> DioExt<'a, M>;
}

impl<'a, M> DioFactoryExt<'a, M>
for ChainAccessorExt<M>
where M: OtherMetadata,
{
    fn dio(&'a mut self) -> DioExt<'a, M> {
        let accessor = ChainAccessorExt::from_accessor(self); 
        let multi = self.multi();
        DioExt {
            accessor: accessor,
            state: Rc::new(RefCell::new(DioState::new())),
            multi: Some(multi),          
        }
    }
}

#[cfg(test)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TestDao
{
    Blah1,
    Blah2(u32),
    Blah3(String),
    Blah4,
}

#[test]
fn test_dio()
{
    let mut chain = create_test_chain("test_dio".to_string());

    let key1;
    let key2;
    {
        let mut dio = chain.dio();

        // Write a value immediately from chain (this data will remain in the transaction)
        {
            let mut dao1 = dio.store(Metadata::default(), TestDao::Blah1);
            key1 = dao1.key().clone();

            // Attempting to load the data object while it is still in scope and not flushed will fail
            match *dio.load(&key1).expect("The data object should have been read") {
                TestDao::Blah1 => {},
                _ => panic!("Data is not saved correctly")
            }

            // When we update this value it will become dirty and hence should block future loads until its flushed or goes out of scope
            *dao1 = TestDao::Blah2(2);
            dio.load::<TestDao>(&key1).expect_err("This load is meant to fail due to a lock being triggered");

            // Flush the data and attempt to read it again (this should succeed)
            dao1.flush().expect("Flush failed");
            match *dio.load(&key1).expect("The dirty data object should have been read after it was flushed") {
                TestDao::Blah2(a) => assert_eq!(a.clone(), 2 as u32),
                _ => panic!("Data is not saved correctly")
            }

            // Again after changing the data reads should fail
            *dao1 = TestDao::Blah3("testblah".to_string());
            dio.load::<TestDao>(&key1).expect_err("This load is meant to fail due to a lock being triggered");

            // Write a record to the chain that we will delete again later
            let dao2 = dio.store(Metadata::default(), TestDao::Blah4);
            key2 = dao2.key().clone();
        }

        // Now its out of scope it should be loadable again
        match &*dio.load(&key1).expect("The dirty data object should have been read after it was flushed") {
            TestDao::Blah3(a) => assert_eq!(a.clone(), "testblah".to_string()),
            _ => panic!("Data is not saved correctly")
        }
    }

    {
        let mut dio = chain.dio();

        // The data we saved earlier should be accessible accross DIO scope boundaries
        match &*dio.load(&key1).expect("The data object should have been read") {
            TestDao::Blah3(a) => assert_eq!(a.clone(), "testblah".to_string()),
            _ => panic!("Data is not saved correctly")
        }

        // First attempt to read the record then delete it
        let dao2 = dio.load::<TestDao>(&key2).expect("The record should load before we delete it in this session");
        dao2.delete();

        // It should no longer load now that we deleted it
        dio.load::<TestDao>(&key2).expect_err("This load should fail as we deleted the record");
    }

    {
        let mut dio = chain.dio();

        // After going out of scope then back again we should still no longer see the record we deleted
        dio.load::<TestDao>(&key2).expect_err("This load should fail as we deleted the record");
    }

    //chain.single().destroy().unwrap();
}
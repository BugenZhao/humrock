use std::{ops::Bound, path::Path, sync::Arc};

use anyhow::{Context, Result};
use rocksdb::{
    ColumnFamily, Direction, IteratorMode, Options, ReadOptions, WriteBatch, WriteOptions,
};
use tokio::sync::mpsc::{channel, Receiver};

#[derive(Clone)]
struct Handle {
    db: Arc<rocksdb::DB>,
}

impl Handle {
    fn new(path: &Path) -> Self {
        let mut options = Options::default();
        options.create_if_missing(true);
        options.set_comparator_with_u64_ts();

        let mut db = rocksdb::DB::open(&options, path).unwrap();
        db.create_cf("humrock", &options).unwrap();
        Self { db: db.into() }
    }

    fn cf_handle(&self) -> &ColumnFamily {
        self.db.cf_handle("humrock").unwrap()
    }
}

pub struct Humrock {
    handle: Handle,
}

impl Humrock {
    pub fn new(path: &Path) -> Self {
        Self {
            handle: Handle::new(path),
        }
    }

    pub async fn get(&self, key: Vec<u8>, epoch: u64) -> Result<Option<Vec<u8>>> {
        let handle = self.handle.clone();

        tokio::task::spawn_blocking(move || {
            let mut read_options = ReadOptions::default();
            read_options.set_timestamp(Some(epoch));

            handle
                .db
                .get_opt(key, &read_options)
                .context("failed to get")
        })
        .await
        .unwrap()
    }

    pub async fn ingest_batch(
        &self,
        batch: impl IntoIterator<Item = (Vec<u8>, Option<Vec<u8>>)> + Send + 'static,
        epoch: u64,
    ) -> Result<usize> {
        let handle = self.handle.clone();

        tokio::task::spawn_blocking(move || {
            let cf = handle.cf_handle();
            let mut write_batch = WriteBatch::default();

            for (key, value) in batch {
                match value {
                    Some(value) => write_batch.put_cf_with_ts(cf, key, value, epoch),
                    None => write_batch.delete_cf_with_ts(cf, key, epoch),
                }
            }
            let size = write_batch.size_in_bytes();

            handle
                .db
                .write_opt(write_batch, &WriteOptions::default())
                .context("failed to write batch")?;
            Ok(size)
        })
        .await
        .unwrap()
    }

    pub fn iter(
        &self,
        epoch: u64,
        start_bound: Bound<Vec<u8>>,
        end_bound: Bound<Vec<u8>>,
        limit: usize,
    ) -> Receiver<Result<(Vec<u8>, Vec<u8>)>> {
        let (tx, rx) = channel(256);
        let handle = self.handle.clone();

        tokio::task::spawn_blocking(move || {
            let cf = handle.cf_handle();
            let mut read_options = ReadOptions::default();
            read_options.set_timestamp(Some(epoch));

            let mode = match &start_bound {
                Bound::Included(start_key) => IteratorMode::From(start_key, Direction::Forward),
                Bound::Excluded(_) => unimplemented!(),
                Bound::Unbounded => IteratorMode::Start,
            };

            let iterator = handle.db.iterator_cf_opt(cf, read_options, mode);

            for result in iterator.take(limit) {
                let result = result
                    .map(|(key, value)| (key.to_vec(), value.to_vec()))
                    .context("failed to iterate next item");

                if let Ok((key, _)) = &result {
                    let ended = match &end_bound {
                        Bound::Included(end) => key > end,
                        Bound::Excluded(end) => key >= end,
                        Bound::Unbounded => false,
                    };
                    if ended {
                        break;
                    }
                }

                let is_err = result.is_err();
                if tx.blocking_send(result).is_err() {
                    break;
                }
                if is_err {
                    break;
                }
            }
        });

        rx
    }
}

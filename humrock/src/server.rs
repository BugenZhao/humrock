use std::ops::Bound;

use futures::{Stream, StreamExt};
use proto::state_store_server::StateStore;
use proto::*;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use crate::humrock::Humrock;

pub struct HumrockService(pub Humrock);

#[tonic::async_trait]
impl StateStore for HumrockService {
    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let request = request.into_inner();

        let value = self
            .0
            .get(request.key, request.epoch)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let response = value
            .map(|value| GetResponse {
                exists: true,
                value,
            })
            .unwrap_or(GetResponse {
                exists: false,
                value: Vec::new(),
            });

        Ok(Response::new(response))
    }

    async fn ingest_batch(
        &self,
        request: Request<IngestBatchRequest>,
    ) -> Result<Response<IngestBatchResponse>, Status> {
        let request = request.into_inner();

        let batch = request
            .kvs
            .into_iter()
            .zip(request.operations)
            .map(|(kv, operation)| {
                (
                    kv.key,
                    (operation == Operation::Put as _).then_some(kv.value),
                )
            });

        let size = self
            .0
            .ingest_batch(batch, request.epoch)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(IngestBatchResponse { size: size as _ }))
    }

    type IterateStream = impl Stream<Item = Result<KeyValue, Status>>;

    async fn iterate(
        &self,
        request: Request<IterateRequest>,
    ) -> Result<Response<Self::IterateStream>, Status> {
        let request = request.into_inner();

        let map_bound = |key_bound: KeyBound| match key_bound.bound_type() {
            BoundType::Excluded => Bound::Excluded(key_bound.key),
            BoundType::Included => Bound::Included(key_bound.key),
            BoundType::Unbounded => Bound::Unbounded,
        };

        let rx = self.0.iter(
            request.epoch,
            map_bound(request.start.unwrap()),
            map_bound(request.end.unwrap()),
            request.limit as _,
        );
        let stream = ReceiverStream::new(rx).map(|result| {
            result
                .map(|(key, value)| KeyValue { key, value })
                .map_err(|e| Status::internal(e.to_string()))
        });

        Ok(Response::new(stream))
    }
}

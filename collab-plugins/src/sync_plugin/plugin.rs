use std::sync::{Arc, Weak};

use collab::core::collab::MutexCollab;
use collab::core::collab_state::SyncState;
use collab::core::origin::CollabOrigin;
use collab::preclude::CollabPlugin;
use collab_define::{CollabObject, CollabType};
use collab_sync_protocol::{ClientUpdateRequest, CollabMessage};
use futures_util::SinkExt;
use tokio_stream::StreamExt;

use tokio_stream::wrappers::WatchStream;
use y_sync::awareness::Awareness;
use y_sync::sync::{Message, SyncMessage};
use yrs::updates::encoder::Encode;

use crate::sync_plugin::client::SinkConfig;
use crate::sync_plugin::client::SyncQueue;

#[derive(Clone, Debug)]
pub struct SyncObject {
  pub object_id: String,
  pub workspace_id: String,
  pub collab_type: CollabType,
  pub device_id: String,
}

impl SyncObject {
  pub fn new(
    object_id: &str,
    workspace_id: &str,
    collab_type: CollabType,
    device_id: &str,
  ) -> Self {
    Self {
      object_id: object_id.to_string(),
      workspace_id: workspace_id.to_string(),
      collab_type,
      device_id: device_id.to_string(),
    }
  }
}

impl From<CollabObject> for SyncObject {
  fn from(collab_object: CollabObject) -> Self {
    Self {
      object_id: collab_object.object_id,
      workspace_id: collab_object.workspace_id,
      collab_type: collab_object.collab_type,
      device_id: collab_object.device_id,
    }
  }
}

pub struct SyncPlugin<Sink, Stream> {
  object: SyncObject,
  sync_queue: Arc<SyncQueue<Sink, Stream>>,
}

impl<E, Sink, Stream> SyncPlugin<Sink, Stream>
where
  E: std::error::Error + Send + Sync + 'static,
  Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
  Stream: StreamExt<Item = Result<CollabMessage, E>> + Send + Sync + Unpin + 'static,
{
  pub fn new(
    origin: CollabOrigin,
    object: SyncObject,
    collab: Weak<MutexCollab>,
    sink: Sink,
    sink_config: SinkConfig,
    stream: Stream,
  ) -> Self {
    let weak_local_collab = collab.clone();
    let sync_queue = SyncQueue::new(object.clone(), origin, sink, stream, collab, sink_config);

    let mut sync_state_stream = WatchStream::new(sync_queue.subscribe_sync_state());
    tokio::spawn(async move {
      while let Some(new_state) = sync_state_stream.next().await {
        if let Some(local_collab) = weak_local_collab.upgrade() {
          if let Some(local_collab) = local_collab.try_lock() {
            local_collab.set_sync_state(new_state);
          }
        }
      }
    });

    Self {
      sync_queue: Arc::new(sync_queue),
      object,
    }
  }

  pub fn subscribe_sync_state(&self) -> WatchStream<SyncState> {
    let rx = self.sync_queue.subscribe_sync_state();
    WatchStream::new(rx)
  }
}

impl<E, Sink, Stream> CollabPlugin for SyncPlugin<Sink, Stream>
where
  E: std::error::Error + Send + Sync + 'static,
  Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
  Stream: StreamExt<Item = Result<CollabMessage, E>> + Send + Sync + Unpin + 'static,
{
  fn did_init(&self, _awareness: &Awareness, _object_id: &str) {
    self.sync_queue.notify(_awareness);
  }

  fn receive_local_update(&self, origin: &CollabOrigin, _object_id: &str, update: &[u8]) {
    let weak_sync_queue = Arc::downgrade(&self.sync_queue);
    let update = update.to_vec();
    let object_id = self.object.object_id.clone();
    let cloned_origin = origin.clone();

    tokio::spawn(async move {
      if let Some(sync_queue) = weak_sync_queue.upgrade() {
        let payload = Message::Sync(SyncMessage::Update(update)).encode_v1();
        sync_queue.queue_msg(|msg_id| {
          ClientUpdateRequest::new(cloned_origin, object_id, msg_id, payload).into()
        });
      }
    });
  }

  fn reset(&self, _object_id: &str) {
    self.sync_queue.clear();
  }
}
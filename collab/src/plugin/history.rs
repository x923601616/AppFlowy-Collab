use crate::plugin::CollabPlugin;
use bytes::Bytes;
use parking_lot::RwLock;
use std::sync::Arc;
use yrs::updates::decoder::Decode;
use yrs::Update;

#[derive(Debug, Default, Clone)]
pub struct CollabHistoryPlugin(Arc<RwLock<Vec<Bytes>>>);

impl CollabHistoryPlugin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_updates(&self) -> Result<Vec<Update>, anyhow::Error> {
        let mut updates = vec![];
        for encoded_data in self.0.read().iter() {
            updates.push(Update::decode_v1(encoded_data)?);
        }
        Ok(updates)
    }
}

impl CollabPlugin for CollabHistoryPlugin {
    fn did_receive_new_update(&self, update: Bytes) {
        self.0.write().push(update);
    }
}

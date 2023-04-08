use collab::preclude::*;
use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

pub struct ChildrenMap {
  pub root: MapRefWrapper,
}

impl Serialize for ChildrenMap {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let txn = self.root.transact();
    let mut map = serializer.serialize_map(Some(self.root.len(&txn) as usize))?;
    for (key, _) in self.root.iter(&txn) {
      // It is save to unwrap here because we know that the key exists
      let children = self.root.get_array_ref_with_txn(&txn, key).unwrap();
      let value = serde_json::json!(children
        .iter(&txn)
        .map(|child| child.to_string(&txn))
        .collect::<Vec<String>>());
      map.serialize_entry(key, &value)?;
    }
    map.end()
  }
}

impl ChildrenMap {
  pub fn new(root: MapRefWrapper) -> Self {
    Self { root }
  }

  pub fn to_json_value(&self) -> serde_json::Value {
    serde_json::to_value(self).unwrap_or_default()
  }

  pub fn get_children_with_txn(
    &self,
    txn: &mut TransactionMut,
    children_id: &str,
  ) -> ArrayRefWrapper {
    self
      .root
      .get_array_ref_with_txn(txn, children_id)
      .unwrap_or_else(|| self.create_children_with_txn(txn, children_id))
  }

  pub fn create_children_with_txn(
    &self,
    txn: &mut TransactionMut,
    children_id: &str,
  ) -> ArrayRefWrapper {
    let children: Vec<String> = vec![];
    self.root.insert_array_with_txn(txn, children_id, children)
  }

  pub fn delete_children_with_txn(&self, txn: &mut TransactionMut, children_id: &str) {
    self.root.delete_with_txn(txn, children_id);
  }

  pub fn get_child_index_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    children_id: &str,
    child_id: &str,
  ) -> Option<u32> {
    let children_ref = self.root.get_array_ref_with_txn(txn, children_id);
    if children_ref.as_ref()?.len(txn) == 0 {
      return None;
    }
    let children_ref = children_ref.unwrap();

    let index = children_ref
      .iter(txn)
      .position(|child| child.to_string(txn) == child_id);

    index.map(|index| index as u32)
  }

  pub fn insert_child_with_txn(
    &self,
    txn: &mut TransactionMut,
    children_id: &str,
    child_id: &str,
    index: u32,
  ) {
    let children_ref = self.get_children_with_txn(txn, children_id);
    children_ref.insert(txn, index, child_id);
  }

  pub fn delete_child_with_txn(&self, txn: &mut TransactionMut, children_id: &str, child_id: &str) {
    let children_ref = self.get_children_with_txn(txn, children_id);
    let index = self.get_child_index_with_txn(txn, children_id, child_id);
    if let Some(index) = index {
      children_ref.remove_with_txn(txn, index);
    }
  }
}

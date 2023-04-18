use std::fmt;
use std::fmt::{Display, Formatter};
use std::ops::Deref;

use collab::preclude::{MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut, YrsValue};
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::database::timestamp;
use crate::id_gen::ID_GEN;
use crate::rows::{Cell, Cells, CellsUpdate};
use crate::views::RowOrder;
use crate::{impl_bool_update, impl_i32_update, impl_i64_update};

#[derive(Copy, Debug, Clone, Eq, PartialEq, Hash)]
pub struct RowId(i64);

impl Display for RowId {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.0.to_string())
  }
}

impl Serialize for RowId {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_str(&self.0.to_string())
  }
}

impl<'de> Deserialize<'de> for RowId {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct RowIdVisitor();

    impl<'de> Visitor<'de> for RowIdVisitor {
      type Value = RowId;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Expected i64 string")
      }

      fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
      where
        E: Error,
      {
        match v.parse::<i64>() {
          Ok(id) => Ok(RowId(id)),
          Err(_) => Err(Error::custom("Expected i64 string")),
        }
      }
    }

    deserializer.deserialize_any(RowIdVisitor())
  }
}

impl Deref for RowId {
  type Target = i64;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl From<i64> for RowId {
  fn from(data: i64) -> Self {
    Self(data)
  }
}

impl From<RowId> for i64 {
  fn from(data: RowId) -> Self {
    data.0
  }
}

impl std::default::Default for RowId {
  fn default() -> Self {
    Self(ID_GEN.lock().next_id())
  }
}

impl AsRef<i64> for RowId {
  fn as_ref(&self) -> &i64 {
    &self.0
  }
}

pub type BlockId = i64;

/// Represents a row in a [Block].
/// A [Row] contains list of [Cell]s. Each [Cell] is associated with a [Field].
/// So the number of [Cell]s in a [Row] is equal to the number of [Field]s.
/// A [Database] contains list of rows that stored in multiple [Block]s.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Row {
  pub id: RowId,
  pub block_id: BlockId,
  pub cells: Cells,
  pub height: i32,
  pub visibility: bool,
  pub created_at: i64,
}

impl Row {
  /// Creates a new instance of [Row]
  /// The default height of a [Row] is 60
  /// The default visibility of a [Row] is true
  /// The default created_at of a [Row] is the current timestamp
  pub fn new<R: Into<RowId>, B: Into<BlockId>>(id: R, block_id: B) -> Self {
    Row {
      id: id.into(),
      block_id: block_id.into(),
      cells: Default::default(),
      height: 60,
      visibility: true,
      created_at: timestamp(),
    }
  }
}

pub struct RowBuilder<'a, 'b> {
  map_ref: MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> RowBuilder<'a, 'b> {
  pub fn new(
    id: RowId,
    block_id: BlockId,
    txn: &'a mut TransactionMut<'b>,
    map_ref: MapRefWrapper,
  ) -> Self {
    map_ref.insert_i64_with_txn(txn, ROW_ID, id);
    map_ref.insert_i64_with_txn(txn, BLOCK_ID, block_id);
    Self { map_ref, txn }
  }

  pub fn update<F>(self, f: F) -> Self
  where
    F: FnOnce(RowUpdate),
  {
    let update = RowUpdate::new(self.txn, &self.map_ref);
    f(update);
    self
  }
  pub fn done(self) {}
}

/// It used to update a [Row]
pub struct RowUpdate<'a, 'b, 'c> {
  map_ref: &'c MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b, 'c> RowUpdate<'a, 'b, 'c> {
  pub fn new(txn: &'a mut TransactionMut<'b>, map_ref: &'c MapRef) -> Self {
    Self { map_ref, txn }
  }

  impl_bool_update!(set_visibility, set_visibility_if_not_none, ROW_VISIBILITY);
  impl_i32_update!(set_height, set_height_at_if_not_none, ROW_HEIGHT);
  impl_i64_update!(set_created_at, set_created_at_if_not_none, CREATED_AT);

  pub fn set_cells(self, cells: Cells) -> Self {
    let cell_map = self.map_ref.get_or_insert_map_with_txn(self.txn, ROW_CELLS);
    cells.fill_map_ref(self.txn, &cell_map);
    self
  }

  pub fn update_cells<F>(self, f: F) -> Self
  where
    F: FnOnce(CellsUpdate),
  {
    let cell_map = self.map_ref.get_or_insert_map_with_txn(self.txn, ROW_CELLS);
    let update = CellsUpdate::new(self.txn, &cell_map);
    f(update);
    self
  }

  pub fn done(self) -> Option<Row> {
    row_from_map_ref(self.map_ref, self.txn)
  }
}

const ROW_ID: &str = "id";
const BLOCK_ID: &str = "bid";
const ROW_VISIBILITY: &str = "visibility";
const ROW_HEIGHT: &str = "height";
const CREATED_AT: &str = "created_at";
const ROW_CELLS: &str = "cells";

/// Return row id and created_at from a [YrsValue]
pub fn row_id_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<(String, i64)> {
  let map_ref = value.to_ymap()?;
  let id = map_ref.get_str_with_txn(txn, ROW_ID)?;
  let crated_at = map_ref
    .get_i64_with_txn(txn, CREATED_AT)
    .unwrap_or_default();
  Some((id, crated_at))
}

/// Return a [RowOrder] and created_at from a [YrsValue]
pub fn row_order_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<(RowOrder, i64)> {
  let map_ref = value.to_ymap()?;
  let id = RowId::from(map_ref.get_i64_with_txn(txn, ROW_ID)?);
  let block_id = map_ref.get_i64_with_txn(txn, BLOCK_ID)?;
  let height = map_ref.get_i64_with_txn(txn, ROW_HEIGHT).unwrap_or(60);
  let crated_at = map_ref
    .get_i64_with_txn(txn, CREATED_AT)
    .unwrap_or_default();
  Some((RowOrder::new(id, block_id, height as i32), crated_at))
}

/// Return a [Row] from a [YrsValue]
pub fn row_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<Row> {
  let map_ref = value.to_ymap()?;
  row_from_map_ref(&map_ref, txn)
}

/// Return a [Cell] in a [Row] from a [YrsValue]
/// The [Cell] is identified by the field_id
pub fn cell_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T, field_id: &str) -> Option<Cell> {
  let cells_map_ref = map_ref.get_map_with_txn(txn, ROW_CELLS)?;
  let cell_map_ref = cells_map_ref.get_map_with_txn(txn, field_id)?;
  Some(Cell::from_map_ref(txn, &cell_map_ref))
}

/// Return a [Row] from a [MapRef]
pub fn row_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<Row> {
  let id = RowId::from(map_ref.get_i64_with_txn(txn, ROW_ID)?);
  let block_id = map_ref.get_i64_with_txn(txn, BLOCK_ID)?;
  let visibility = map_ref
    .get_bool_with_txn(txn, ROW_VISIBILITY)
    .unwrap_or(true);

  let height = map_ref.get_i64_with_txn(txn, ROW_HEIGHT).unwrap_or(60);

  let created_at = map_ref
    .get_i64_with_txn(txn, CREATED_AT)
    .unwrap_or_else(|| chrono::Utc::now().timestamp());

  let cells = map_ref
    .get_map_with_txn(txn, ROW_CELLS)
    .map(|map_ref| (txn, &map_ref).into())
    .unwrap_or_default();

  Some(Row {
    id,
    block_id,
    cells,
    height: height as i32,
    visibility,
    created_at,
  })
}

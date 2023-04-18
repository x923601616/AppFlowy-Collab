#![allow(clippy::all)]

use std::ops::Deref;
use std::sync::Arc;

use collab_database::block::CreateRowParams;
use collab_database::database::DuplicatedDatabase;
use collab_database::fields::Field;
use collab_database::rows::{Cells, CellsBuilder, RowId};
use collab_database::user::UserDatabase as InnerUserDatabase;
use collab_database::views::CreateDatabaseParams;
use collab_persistence::CollabKV;
use parking_lot::Mutex;
use serde_json::Value;

use crate::helper::{make_kv_db, TestTextCell};

pub enum DatabaseScript {
  CreateDatabase {
    params: CreateDatabaseParams,
  },
  CreateRow {
    database_id: String,
    params: CreateRowParams,
  },
  EditRow {
    database_id: String,
    row_id: RowId,
    cells: Cells,
  },
  AssertDatabase {
    database_id: String,
    expected: Value,
  },
  AssertNumOfUpdates {
    oid: String,
    expected: usize,
  },
  IsExist {
    oid: String,
    expected: bool,
  },
}

#[derive(Clone)]
pub struct DatabaseTest {
  pub kv: Arc<CollabKV>,
  pub user_database: UserDatabase,
}

pub fn database_test() -> DatabaseTest {
  DatabaseTest::new()
}

impl DatabaseTest {
  pub fn new() -> Self {
    let kv = make_kv_db();
    let inner = InnerUserDatabase::new(1, kv.clone());
    let user_database = UserDatabase(Arc::new(Mutex::new(inner)));
    Self { kv, user_database }
  }

  #[allow(dead_code)]
  pub fn get_database_data(&self, database_id: &str) -> DuplicatedDatabase {
    let database = self.user_database.lock().get_database(database_id).unwrap();
    database.duplicate_database()
  }

  pub async fn run_scripts(&mut self, scripts: Vec<DatabaseScript>) {
    let mut handles = vec![];
    for script in scripts {
      let user_database = self.user_database.clone();
      let db = self.kv.clone();
      let handle = tokio::spawn(async move {
        run_script(user_database, db, script);
      });
      handles.push(handle);
    }
    for result in futures::future::join_all(handles).await {
      assert!(result.is_ok());
    }
  }
}

pub fn run_script(user_database: UserDatabase, db: Arc<CollabKV>, script: DatabaseScript) {
  match script {
    DatabaseScript::CreateDatabase { params } => {
      user_database.lock().create_database(params).unwrap();
    },
    DatabaseScript::CreateRow {
      database_id,
      params,
    } => {
      user_database
        .lock()
        .get_database(&database_id)
        .unwrap()
        .create_row(params);
    },
    DatabaseScript::EditRow {
      database_id,
      row_id,
      cells,
    } => {
      user_database
        .lock()
        .get_database(&database_id)
        .unwrap()
        .update_row(row_id, |row| {
          row.set_cells(cells);
        });
    },
    // DatabaseScript::CreateField { database_id, field } => {
    //   user_database
    //     .lock()
    //     .get_database(&database_id)
    //     .unwrap()
    //     .create_field(field);
    // },
    DatabaseScript::AssertDatabase {
      database_id,
      expected,
    } => {
      let inner = InnerUserDatabase::new(1, db);
      let database = inner.get_database(&database_id).unwrap();
      let actual = database.to_json_value();
      assert_json_diff::assert_json_eq!(actual, expected);
    },
    DatabaseScript::IsExist {
      oid: database_id,
      expected,
    } => {
      assert_eq!(db.doc(1).is_exist(&database_id), expected,)
    },
    DatabaseScript::AssertNumOfUpdates {
      oid: database_id,
      expected,
    } => {
      let updates = db.doc(1).get_updates(&database_id).unwrap();
      assert_eq!(updates.len(), expected,);
    },
  }
}

pub fn create_database(database_id: &str) -> CreateDatabaseParams {
  let row_1 = CreateRowParams {
    id: 1.into(),
    cells: CellsBuilder::new()
      .insert_cell("f1", TestTextCell::from("1f1cell"))
      .insert_cell("f2", TestTextCell::from("1f2cell"))
      .insert_cell("f3", TestTextCell::from("1f3cell"))
      .build(),
    height: 0,
    visibility: true,
    prev_row_id: None,
    timestamp: 0,
  };
  let row_2 = CreateRowParams {
    id: 2.into(),
    cells: CellsBuilder::new()
      .insert_cell("f1", TestTextCell::from("2f1cell"))
      .insert_cell("f2", TestTextCell::from("2f2cell"))
      .build(),
    height: 0,
    visibility: true,
    prev_row_id: None,
    timestamp: 0,
  };
  let row_3 = CreateRowParams {
    id: 3.into(),
    cells: CellsBuilder::new()
      .insert_cell("f1", TestTextCell::from("3f1cell"))
      .insert_cell("f3", TestTextCell::from("3f3cell"))
      .build(),
    height: 0,
    visibility: true,
    prev_row_id: None,
    timestamp: 0,
  };
  let field_1 = Field::new("f1".to_string(), "text field".to_string(), 0, true);
  let field_2 = Field::new("f2".to_string(), "single select field".to_string(), 2, true);
  let field_3 = Field::new("f3".to_string(), "checkbox field".to_string(), 1, true);

  CreateDatabaseParams {
    database_id: database_id.to_string(),
    view_id: "v1".to_string(),
    name: "my first database".to_string(),
    layout: Default::default(),
    layout_settings: Default::default(),
    filters: vec![],
    groups: vec![],
    sorts: vec![],
    created_rows: vec![row_1, row_2, row_3],
    fields: vec![field_1, field_2, field_3],
  }
}

#[derive(Clone)]
pub struct UserDatabase(Arc<Mutex<InnerUserDatabase>>);

impl Deref for UserDatabase {
  type Target = Arc<Mutex<InnerUserDatabase>>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

unsafe impl Sync for UserDatabase {}

unsafe impl Send for UserDatabase {}

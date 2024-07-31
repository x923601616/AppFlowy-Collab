use collab_document::{
  blocks::Block, conversions::convert_document_to_plain_text, document::Document,
};
use nanoid::nanoid;

use crate::util::{insert_block, DocumentTest};

#[tokio::test]
async fn plain_text_1_test() {
  let doc_id = "1";
  let test = DocumentTest::new(1, doc_id).await;
  let document = test.document;
  let paragraphs = vec![
    "Welcome to AppFlowy!".to_string(),
    "Here are the basics".to_string(),
    "Click anywhere and just start typing.".to_string(),
    "Highlight any text, and use the editing menu to _style_ **your** <u>writing</u> `however` you ~~like.~~".to_string(),
    "As soon as you type `/` a menu will pop up. Select different types of content blocks you can add.".to_string(),
    "Type `/` followed by `/bullet` or `/num` to create a list.".to_string(),
    "Click `+ New Page `button at the bottom of your sidebar to add a new page.".to_string(),
    "Click `+` next to any page title in the sidebar to quickly add a new subpage, `Document`, `Grid`, or `Kanban Board`.".to_string(),
  ];
  insert_paragraphs(&document, paragraphs.clone());

  let plain_text = convert_document_to_plain_text(document).unwrap();
  let mut splitted = plain_text.split('\n').collect::<Vec<&str>>();
  // the first one and the last one are empty
  assert_eq!(splitted.len(), 10);
  splitted.remove(0);
  splitted.pop();

  for i in 0..splitted.len() {
    assert_eq!(splitted[i], paragraphs[i]);
  }
}

fn insert_paragraphs(document: &Document, paragraphs: Vec<String>) {
  let page_id = document.get_page_id().unwrap();
  let mut prev_id = "".to_string();
  for paragraph in paragraphs {
    let block_id = nanoid!(6);
    let text_id = nanoid!(6);
    let block = Block {
      id: block_id.clone(),
      ty: "paragraph".to_owned(),
      parent: page_id.clone(),
      children: "".to_string(),
      external_id: Some(text_id.clone()),
      external_type: Some("text".to_owned()),
      data: Default::default(),
    };

    insert_block(document, block, prev_id).unwrap();

    prev_id = block_id.clone();

    document.create_text(&text_id, format!(r#"[{{"insert": "{}"}}]"#, paragraph));
  }
}

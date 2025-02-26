/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use bson::Document;
use bson::spec::ElementType;
use serde::{Deserialize, Serialize};
use polodb_core::{Database, Result};
use polodb_core::bson::{doc, Bson};

mod common;

use common::prepare_db;
use polodb_core::test_utils::{mk_db_path, mk_journal_path};

#[derive(Debug, Serialize, Deserialize)]
struct Book {
    title: String,
    author: String,
}

#[test]
fn test_insert_struct() {
    vec![
        prepare_db("test-insert-struct").unwrap(),
        Database::open_memory().unwrap(),
    ].iter().for_each(|db| {
        // Get a handle to a collection of `Book`.
        let typed_collection = db.collection::<Book>("books");

        let books = vec![
            Book {
                title: "The Grapes of Wrath".to_string(),
                author: "John Steinbeck".to_string(),
            },
            Book {
                title: "To Kill a Mockingbird".to_string(),
                author: "Harper Lee".to_string(),
            },
        ];

        // Insert the books into "mydb.books" collection, no manual conversion to BSON necessary.
        typed_collection.insert_many(books).unwrap();

        let book = typed_collection.find_one(doc! {
            "title": "The Grapes of Wrath",
        }).unwrap().unwrap();
        assert_eq!(book.author, "John Steinbeck");

        let cursor = typed_collection.find(doc! {
            "$or": [
                {
                    "title": "The Grapes of Wrath",
                },
                {
                    "title": "To Kill a Mockingbird",
                }
            ]
        }).unwrap();
        let result = cursor.collect::<Result<Vec<Book>>>().unwrap();
        assert_eq!(result.len(), 2);
    });
}

#[test]
fn test_insert_bigger_key() {
    vec![
        prepare_db("test-insert-bigger-key").unwrap(),
        Database::open_memory().unwrap(),
    ].iter().for_each(|db| {
        let collection = db.collection("test");
        let mut doc = doc! {};

        let mut new_str: String = String::new();
        for _i in 0..32 {
            new_str.push('0');
        }

        doc.insert::<String, Bson>("_id".into(), Bson::String(new_str.clone()));

        let _ = collection.insert_one(doc).unwrap();
    });
}

#[test]
fn test_very_large_binary() {
    let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    d.pop();
    d.pop();
    d.push("fixtures/test_img.jpg");

    let mut file = File::open(d).unwrap();

    let mut data = Vec::new();
    file.read_to_end(&mut data).unwrap();

    println!("data size: {}", data.len());
    vec![
        prepare_db("test-very-large-data").unwrap(),
        Database::open_memory().unwrap(),
    ].iter().for_each(|db| {
        let collection = db.collection("test");

        let mut doc = doc! {};
        let origin_data = data.clone();
        doc.insert::<String, Bson>("content".into(), Bson::Binary(bson::Binary {
            subtype: bson::spec::BinarySubtype::Generic,
            bytes: origin_data.clone(),
        }));

        let result = collection.insert_one(doc).unwrap();

        let new_id = result.inserted_id;
        let back  = collection.find_one(doc! {
            "_id": new_id,
        }).unwrap().unwrap();

        let back_bin = back.get("content").unwrap();

        let binary = match back_bin {
            Bson::Binary(bin) => {
                bin
            }
            _ => panic!("type unmatched"),
        };
        assert_eq!(&binary.bytes, &origin_data);
    });
}

#[test]
fn test_insert_after_delete() {
    vec![
        prepare_db("test-insert-after-delete").unwrap(),
        Database::open_memory().unwrap(),
    ].iter().for_each(|db| {

        let collection = db.collection::<Document>("test");

        let mut doc_collection  = vec![];

        for i in 0..1000 {
            let content = i.to_string();
            let new_doc = doc! {
                    "_id": content.clone(),
                    "content": content,
                };
            doc_collection.push(new_doc);
        }
        collection.insert_many(&doc_collection).unwrap();

        let result = collection.delete_one(doc! {
            "_id": "500",
        }).unwrap();
        assert_eq!(result.deleted_count, 1);

        collection.insert_one(doc! {
            "_id": "500",
            "content": "Hello World",
        }).unwrap();

        let one = collection.find_one(doc! {
            "_id": "500",
        }).unwrap().unwrap();

        assert_eq!(one.get("content").unwrap().as_str().unwrap(), "Hello World");
    });
}

#[test]
fn test_insert_different_types_as_key() {
    let db = Database::open_memory().unwrap();
    let collection = db.collection::<Document>("test");

    collection.insert_one(doc! {
        "_id": 0,
    }).unwrap();

    collection.insert_one(doc! {
        "_id": "0",
    }).unwrap();

    let cursor = collection.find(None).unwrap();
    let result: Vec<Result<Document>> = cursor.collect();
    assert_eq!(result.len(), 2);

    assert_eq!(result[0].as_ref().unwrap().get("_id").unwrap().element_type(), ElementType::String);
    assert_eq!(result[1].as_ref().unwrap().get("_id").unwrap().element_type(), ElementType::Int32);
}

#[test]
fn test_insert_persist() {
    const NAME: &str = "test-insert-persist";
    let db_path = mk_db_path(NAME);
    let journal_path = mk_journal_path(NAME);

    let _ = std::fs::remove_file(db_path.as_path());
    let _ = std::fs::remove_file(journal_path);

    // Open the database for 10 times
    for i in 0..10 {
        let db = Database::open_file(&db_path).unwrap();

        let collection = db.collection::<Document>("test");
        let len = collection.count_documents().unwrap();
        assert_eq!(len, i as u64);
        let document = doc! {
            "test": "test",
        };
        collection.insert_one(document).unwrap();
        let result = collection.find(None).unwrap().collect::<Result<Vec<_>>>().unwrap();
        assert_eq!(result.len() as u64, i as u64 + 1);
    }
}

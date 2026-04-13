//! Integration test: `#[derive(Schema)]` + `Repo::insert` with no JSON intermediate.

use aethos_orm::{Repo, Schema, SqlValue};

#[derive(Debug, Schema, sqlx::FromRow)]
#[schema(table = "products")]
struct Product {
    #[field(primary_key)]
    pub id:    i64,
    pub name:  String,
    pub price: f64,
    pub stock: i32,
}

#[tokio::test]
async fn schema_derive_insert_and_fetch() {
    let repo = Repo::<sqlx::Sqlite>::connect(":memory:").await.unwrap();
    repo.execute(
        "CREATE TABLE products (id INTEGER PRIMARY KEY, name TEXT NOT NULL, price REAL, stock INTEGER)"
    ).await.unwrap();

    let p = Product { id: 0, name: "Widget".into(), price: 9.99, stock: 42 };

    // Verify derive generated correct metadata
    assert_eq!(Product::table_name(), "products");
    assert_eq!(Product::primary_key(), "id");
    assert_eq!(Product::columns(), &["name", "price", "stock"]);

    // Verify to_row_values() — direct field conversion, no JSON
    let vals = p.to_row_values();
    assert_eq!(vals.len(), 3);
    assert!(matches!(&vals[0], SqlValue::Text(s) if s == "Widget"));
    assert!(matches!(&vals[1], SqlValue::Float(_)));
    assert!(matches!(&vals[2], SqlValue::Int(42)));

    // Full round-trip: insert → fetch
    let id = repo.insert(&p).await.unwrap();
    assert!(id > 0);

    let fetched: Product = repo.get("products", id).await.unwrap();
    assert_eq!(fetched.name, "Widget");
    assert!((fetched.price - 9.99).abs() < 0.001);
    assert_eq!(fetched.stock, 42);
}

#[tokio::test]
async fn schema_optional_fields() {
    #[derive(Debug, Schema, sqlx::FromRow)]
    #[schema(table = "notes")]
    struct Note {
        #[field(primary_key)]
        pub id:   i64,
        pub body: String,
        pub tag:  Option<String>,
    }

    let repo = Repo::<sqlx::Sqlite>::connect(":memory:").await.unwrap();
    repo.execute(
        "CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT NOT NULL, tag TEXT)"
    ).await.unwrap();

    let n = Note { id: 0, body: "hello".into(), tag: None };
    let vals = n.to_row_values();
    assert!(matches!(&vals[1], SqlValue::Null));

    let id = repo.insert(&n).await.unwrap();
    let fetched: Note = repo.get("notes", id).await.unwrap();
    assert_eq!(fetched.body, "hello");
    assert!(fetched.tag.is_none());
}

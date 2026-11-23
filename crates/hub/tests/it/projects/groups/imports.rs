//! G3: the migration boundary owns concurrency, receipts and same-hub acknowledgement.

use super::{Kit, audit_count, create, groups, imported, kit, set};
use serde_json::{Value, json};
use sqlx::PgPool;

async fn import(kit: &Kit, source: &str, name: &str, projects: &[&String]) -> Value {
    kit.post(
        "mutation($input: ProjectGroupImportInput!) { importProjectGroup(input: $input) }",
        json!({ "input": { "source": source, "name": name, "projects": projects } }),
    )
    .await
}

#[sqlx::test(migrations = "./migrations")]
async fn same_name_import_keeps_assigned_projects_and_acknowledges_only_this_hub(pool: PgPool) {
    let kit = kit(&pool).await;
    let [site, blog, docs] = imported(&kit).await;
    create(&kit, "Work", &[&site]).await;
    create(&kit, "Home", &[&blog]).await;
    let foreign = uuid::Uuid::now_v7().to_string();
    let source = uuid::Uuid::now_v7().to_string();
    let answer = import(&kit, &source, "WORK", &[&site, &blog, &docs, &foreign]).await;
    let mut acknowledged: Vec<String> =
        serde_json::from_value(answer["data"]["importProjectGroup"].clone()).unwrap();
    acknowledged.sort();
    let mut expected = vec![site, blog, docs];
    expected.sort();
    assert_eq!(acknowledged, expected);
    assert_eq!(
        groups(&kit).await,
        [("Home".to_owned(), 1), ("Work".to_owned(), 2)]
    );
    assert_eq!(kit.project("blog").await["group"]["name"], "Home");
    assert_eq!(kit.project("docs").await["group"]["name"], "Work");
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_clients_reuse_one_name_and_retry_cannot_restore_removed_members(pool: PgPool) {
    let kit = kit(&pool).await;
    let [site, blog, _] = imported(&kit).await;
    let first = uuid::Uuid::now_v7().to_string();
    let second = uuid::Uuid::now_v7().to_string();
    let members = [&site, &blog];
    let (a, b) = tokio::join!(
        import(&kit, &first, "Work", &members),
        import(&kit, &second, "WORK", &members)
    );
    for answer in [&a, &b] {
        assert_eq!(
            answer["data"]["importProjectGroup"]
                .as_array()
                .unwrap()
                .len(),
            2,
            "{answer}"
        );
    }
    assert_eq!(groups(&kit).await.len(), 1);
    set(&kit, &[&site], None).await;
    import(&kit, &first, "Work", &members).await;
    import(&kit, &second, "WORK", &members).await;
    assert!(kit.project("site").await["group"].is_null());
    assert_eq!(groups(&kit).await[0].1, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn receipts_survive_rename_and_delete_and_empty_foreign_groups_do_not_leak(pool: PgPool) {
    let kit = kit(&pool).await;
    let [site, blog, docs] = imported(&kit).await;
    let source = uuid::Uuid::now_v7().to_string();
    let foreign = uuid::Uuid::now_v7().to_string();
    for members in [vec![], vec![&foreign]] {
        let answer = import(&kit, &source, "Foreign", &members).await;
        assert_eq!(answer["data"]["importProjectGroup"], json!([]));
    }
    assert_eq!(groups(&kit).await, Vec::<(String, i64)>::new());
    import(&kit, &source, "Work", &[&site]).await;
    let id = kit.project("site").await["group"]["id"].clone();
    kit.post(
        "mutation($id: ID!) { renameProjectGroup(id: $id, name: \"Renamed\") { id } }",
        json!({ "id": id }),
    )
    .await;
    import(&kit, &source, "Work", &[&blog]).await;
    assert_eq!(groups(&kit).await, [("Renamed".to_owned(), 2)]);
    kit.post(
        "mutation($id: ID!) { deleteProjectGroup(id: $id) }",
        json!({ "id": id }),
    )
    .await;
    let answer = import(&kit, &source, "Work", &[&site, &blog, &docs]).await;
    assert_eq!(
        answer["data"]["importProjectGroup"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(groups(&kit).await, Vec::<(String, i64)>::new());
    assert!(kit.project("docs").await["group"].is_null());
}

async fn empty(kit: &Kit, source: &str, name: &str) -> Value {
    kit.post(
        "mutation($input: ProjectGroupImportInput!) { importProjectGroup(input: $input) }",
        json!({ "input": { "source": source, "name": name, "projects": [], "includeEmpty": true } }),
    ).await
}

#[sqlx::test(migrations = "./migrations")]
async fn empty_import_is_explicit_and_receipts_preserve_rename_and_delete(pool: PgPool) {
    let kit = kit(&pool).await;
    imported(&kit).await;
    let source = uuid::Uuid::now_v7().to_string();
    let foreign = uuid::Uuid::now_v7().to_string();
    let answer = kit.post(
        "mutation($input: ProjectGroupImportInput!) { importProjectGroup(input: $input) }",
        json!({ "input": { "source": source, "name": "Foreign", "projects": [foreign], "includeEmpty": true } }),
    ).await;
    assert_eq!(answer["data"]["importProjectGroup"], json!([]));
    assert_eq!(groups(&kit).await, Vec::<(String, i64)>::new());
    let (first, retry) = tokio::join!(empty(&kit, &source, "Work"), empty(&kit, &source, "Work"));
    assert_eq!(first["data"]["importProjectGroup"], json!([]));
    assert_eq!(retry["data"]["importProjectGroup"], json!([]));
    assert_eq!(groups(&kit).await, [("Work".to_owned(), 0)]);
    assert_eq!(audit_count(&pool).await, 1);
    let second = uuid::Uuid::now_v7().to_string();
    empty(&kit, &second, "WORK").await;
    assert_eq!(groups(&kit).await, [("Work".to_owned(), 0)]);
    let listed = kit.post("{ projectGroups { id } }", json!({})).await;
    let id = listed["data"]["projectGroups"][0]["id"].clone();
    kit.post(
        "mutation($id: ID!) { renameProjectGroup(id: $id, name: \"Renamed\") { id } }",
        json!({ "id": id }),
    )
    .await;
    empty(&kit, &source, "Work").await;
    assert_eq!(groups(&kit).await, [("Renamed".to_owned(), 0)]);
    kit.post(
        "mutation($id: ID!) { deleteProjectGroup(id: $id) }",
        json!({ "id": id }),
    )
    .await;
    let count = audit_count(&pool).await;
    empty(&kit, &source, "Work").await;
    empty(&kit, &second, "WORK").await;
    assert_eq!(groups(&kit).await, Vec::<(String, i64)>::new());
    assert_eq!(audit_count(&pool).await, count);
}

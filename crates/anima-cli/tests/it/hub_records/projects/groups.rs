//! Project group command boundaries.

use super::*;

#[tokio::test]
async fn pro_060_project_group_add_names_the_group_by_name_and_the_projects_by_slug() {
    let hub = FakeHub::start().await;
    hub.answer(
        "ProjectGroups",
        data(
            json!({ "projectGroups": [{ "id": "group-1", "name": "Work", "projectCount": 0,
                                          "createdAt": "2026-11-20T10:00:00Z" }] }),
        ),
    );
    hub.answer(
        "ProjectIdBySlug",
        data(json!({ "project": { "id": "project-1" } })),
    );
    hub.answer(
        "ProjectsGroupSet",
        data(json!({ "setProjectsGroup": ["project-1"] })),
    );
    let run = signed_in()
        .run(&[
            "project",
            "group",
            "add",
            "work",
            "anima",
            "--hub",
            &hub.origin,
        ])
        .await;
    assert_eq!(run.code, 0, "{run:?}");
    assert!(run.stdout.contains("project-1"), "{run:?}");
    let set: Vec<Value> = hub
        .seen()
        .into_iter()
        .filter(|seen| seen.operation == "ProjectsGroupSet")
        .map(|seen| seen.variables)
        .collect();
    assert_eq!(
        set,
        [json!({ "projects": ["project-1"], "group": "group-1" })]
    );

    let run = signed_in()
        .run(&[
            "project",
            "group",
            "add",
            "home",
            "anima",
            "--hub",
            &hub.origin,
        ])
        .await;
    assert_ne!(run.code, 0, "{run:?}");
    assert!(
        run.stderr.contains("no project group named home"),
        "{run:?}"
    );
}

#[tokio::test]
async fn g3_group_import_sends_stable_source_and_raw_project_ids() {
    let hub = FakeHub::start().await;
    hub.answer(
        "ProjectGroupImport",
        data(json!({ "importProjectGroup": ["p-1"] })),
    );
    let run = signed_in()
        .run(&[
            "project",
            "group",
            "import",
            "device-group",
            "Work",
            "p-1",
            "foreign",
            "--hub",
            &hub.origin,
        ])
        .await;
    assert_eq!(run.code, 0, "{run:?}");
    assert!(run.stdout.contains("p-1"), "{run:?}");
    assert!(!run.stdout.contains("foreign"), "{run:?}");
    let imports: Vec<Value> = hub
        .seen()
        .into_iter()
        .filter(|seen| seen.operation == "ProjectGroupImport")
        .map(|seen| seen.variables)
        .collect();
    assert_eq!(
        imports,
        [
            json!({ "input": { "source": "device-group", "name": "Work", "projects": ["p-1", "foreign"] } })
        ]
    );
}

#[tokio::test]
async fn g3_empty_import_requires_a_hub_that_understands_empty_groups() {
    let hub = FakeHub::start().await;
    hub.answer(
        "ProjectGroupImport",
        data(json!({ "importProjectGroup": [] })),
    );
    let run = signed_in()
        .run(&[
            "project",
            "group",
            "import",
            "device-group",
            "Empty",
            "--hub",
            &hub.origin,
        ])
        .await;
    assert_eq!(run.code, 0, "{run:?}");
    let sent = hub.seen();
    let import = sent
        .iter()
        .find(|seen| seen.operation == "ProjectGroupImport")
        .unwrap();
    assert_eq!(
        import.variables,
        json!({ "input": { "source": "device-group", "name": "Empty", "projects": [], "includeEmpty": true } })
    );
}

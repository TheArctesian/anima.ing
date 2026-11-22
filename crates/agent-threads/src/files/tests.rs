use super::*;

fn found(paths: &[(&str, bool)]) -> Vec<FoundPath> {
    paths
        .iter()
        .map(|(path, folder)| FoundPath {
            path: (*path).to_owned(),
            folder: *folder,
        })
        .collect()
}

fn paths(list: &[FoundPath]) -> Vec<&str> {
    list.iter().map(|entry| entry.path.as_str()).collect()
}

#[test]
fn th108_secrets_and_folders_of_tools_are_left_out() {
    for (path, folder) in [
        (".env", false),
        ("apps/web/.env.local", false),
        ("certs/server.pem", false),
        ("deploy/secrets/hub.toml", false),
        ("deploy/Secrets", true),
        (".git", true),
        ("apps/client/node_modules", true),
        ("target", true),
    ] {
        assert!(left_out(path, folder), "{path}");
    }
    for (path, folder) in [
        (".env.example", false),
        ("src/main.rs", false),
        (".github", true),
        ("src/target.rs", false),
        ("docs/secrets.md", false),
    ] {
        assert!(!left_out(path, folder), "{path}");
    }
}

#[test]
fn th108_the_best_match_comes_first_and_the_answer_is_held_to_most() {
    let all = found(&[
        ("README.md", false),
        ("src", true),
        ("src/composer.ts", false),
        ("src/composer", true),
        ("src/lib/my-composer.ts", false),
        ("src/lib/decomposer.ts", false),
        ("src/composer/engine.ts", false),
        ("docs/c-o-m-p.md", false),
    ]);
    assert_eq!(
        paths(&ranked(&all, " Composer ", 10)),
        [
            "src/composer",
            "src/composer.ts",
            "src/lib/my-composer.ts",
            "src/lib/decomposer.ts",
            "src/composer/engine.ts",
        ]
    );
    assert_eq!(paths(&ranked(&all, "rmd", 10)), ["README.md"]);
    assert_eq!(paths(&ranked(&all, "composer", 2)).len(), 2);
    assert!(ranked(&all, "zzz", 10).is_empty());
}

#[test]
fn th108_an_empty_query_lists_the_walk_from_the_top() {
    let all = found(&[("a", true), ("b.rs", false), ("a/c.rs", false)]);
    assert_eq!(paths(&ranked(&all, "", 2)), ["a", "b.rs"]);
    let many: Vec<FoundPath> = (0..MOST_FOUND + 50)
        .map(|n| FoundPath {
            path: format!("f{n}"),
            folder: false,
        })
        .collect();
    assert_eq!(ranked(&many, "", usize::MAX).len(), MOST_FOUND);
}

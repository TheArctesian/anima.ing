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

fn ranked_paths(found: &[FoundPath], query: &str, most: usize) -> Vec<FoundPath> {
    ranked(found, query, most).0
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
        ("config/.env", true),
        ("aws/credentials", true),
        ("id_rsa.d", true),
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
        paths(&ranked_paths(&all, " Composer ", 10)),
        [
            "src/composer",
            "src/composer.ts",
            "src/lib/my-composer.ts",
            "src/lib/decomposer.ts",
            "src/composer/engine.ts",
        ]
    );
    assert_eq!(paths(&ranked_paths(&all, "rmd", 10)), ["README.md"]);
    assert_eq!(paths(&ranked_paths(&all, "composer", 2)).len(), 2);
    assert_eq!(ranked(&all, "zzz", 10), (Vec::new(), false));
    assert!(ranked(&all, "composer", 2).1, "more matched");
    assert!(!ranked(&all, "composer", 5).1);
}

#[test]
fn th108_an_empty_query_lists_the_top_folders_first_then_the_files() {
    let all = found(&[
        ("b.rs", false),
        ("z", true),
        ("a", true),
        ("a/c.rs", false),
        ("A.md", false),
    ]);
    assert_eq!(
        paths(&ranked_paths(&all, "  ", 10)),
        ["a", "z", "A.md", "b.rs"]
    );
    let (first, more) = ranked(&all, "", 1);
    assert_eq!(paths(&first), ["a"]);
    assert!(more);
}

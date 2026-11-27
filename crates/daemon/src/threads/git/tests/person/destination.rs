use super::{Bench, GitAction, GitOutcome, GitRefusal, PERSON, PUSH, commit, refused};

#[tokio::test]
async fn a_push_receipt_applies_only_to_the_destination_that_received_the_commit() {
    let bench = Bench::new().await;
    bench.with_origin().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    assert!(matches!(
        bench.call(PERSON, commit("change", None, None)).await,
        GitOutcome::Committed { .. }
    ));
    let first = bench.call(PERSON, PUSH).await;
    let GitOutcome::Pushed {
        branch,
        up_to_date: false,
        ..
    } = first
    else {
        panic!("no first push: {first:?}");
    };

    // Page preparation performs no network I/O, even for a forge URL.
    let url = "remote.origin.pushurl";
    bench
        .git_in(
            &folder,
            &["config", url, "https://github.com/example/fixture.git"],
        )
        .await;
    let new_page = bench.call(PERSON, GitAction::PullRequest).await;
    assert_eq!(refused(&new_page), Some(&GitRefusal::NotPushed));

    let second = bench.project.with_file_name("second.git");
    std::fs::create_dir(&second).expect("second local destination");
    bench.git_in(&second, &["init", "--quiet", "--bare"]).await;
    bench
        .git_in(&folder, &["config", url, second.to_str().expect("path")])
        .await;
    assert_eq!(bench.trust(PERSON).await, GitOutcome::Trusted);
    let second_push = bench.call(PERSON, PUSH).await;
    assert!(
        matches!(
            second_push,
            GitOutcome::Pushed {
                up_to_date: false,
                ..
            }
        ),
        "{second_push:?}"
    );
    assert_eq!(
        bench
            .git_in(&second, &["log", "--format=%s", &branch])
            .await,
        "change\nfirst"
    );
}

#[tokio::test]
async fn tc7_push_pins_the_consent_destination_even_with_global_url_rewrites() {
    let bench = Bench::new().await;
    bench.with_origin().await;
    let folder = bench.worktree().await;
    let origin = bench.project.with_file_name("origin.git");
    let other = bench.project.with_file_name("rewritten.git");
    std::fs::create_dir(&other).expect("second local destination");
    bench.git_in(&other, &["init", "--quiet", "--bare"]).await;
    let global = bench.project.with_file_name("person").join(".gitconfig");
    std::fs::write(
        &global,
        format!(
            "[url \"{}\"]\n insteadOf = {}\n pushInsteadOf = {}\n",
            other.display(),
            origin.display(),
            origin.display(),
        ),
    )
    .expect("synthetic global configuration");
    std::fs::write(folder.join("a.txt"), "new change\n").expect("work");
    assert!(matches!(
        bench.call(PERSON, commit("pin", None, None)).await,
        GitOutcome::Committed { .. }
    ));
    let status = bench.status(PERSON).await;
    assert_eq!(status.push_url.as_deref(), origin.to_str());
    let outcome = bench.call(PERSON, PUSH).await;
    assert!(
        matches!(&outcome, GitOutcome::Pushed { url, .. } if Some(url.as_str()) == origin.to_str()),
        "{outcome:?}"
    );
    let names = ["for-each-ref", "--format=%(refname)"];
    assert!(
        bench
            .git_in(&origin, &names)
            .await
            .contains("refs/heads/anima/12345678")
    );
    assert_eq!(bench.git_in(&other, &names).await, "");
}

use super::*;

async fn lines(bytes: &[u8]) -> Vec<String> {
    let mut reader = BufReader::with_capacity(16, bytes);
    let mut found = Vec::new();
    while let Some(line) = next_line(&mut reader).await.expect("reads") {
        found.push(line);
    }
    found
}

#[tokio::test]
async fn thr_113_whole_lines_come_out_also_when_they_cross_the_buffer() {
    let found = lines(b"first line that is long\r\nsecond\nlast with no end").await;
    assert_eq!(
        found,
        ["first line that is long", "second", "last with no end"]
    );
}

#[tokio::test]
async fn thr_046_a_line_over_the_limit_is_read_to_its_end_and_marked() {
    let mut bytes = vec![b'x'; MAX_LINE + 10];
    bytes.extend_from_slice(b"\nnext\n");
    assert_eq!(lines(&bytes).await, [TOO_LONG, "next"]);
}

#[test]
fn thr_040_a_start_needs_absolute_paths_and_no_nul_byte() {
    let args = vec!["-p".to_owned()];
    let env = vec![("HOME".to_owned(), "/home/a".to_owned())];
    let good = Start {
        program: "/bin/cat",
        args: &args,
        env: &env,
        folder: "/tmp",
    };
    assert!(good.fits());
    assert!(
        !Start {
            program: "cat",
            ..good
        }
        .fits()
    );
    assert!(
        !Start {
            folder: "work",
            ..good
        }
        .fits()
    );
    let bad_env = vec![("A=B".to_owned(), "x".to_owned())];
    assert!(
        !Start {
            env: &bad_env,
            ..good
        }
        .fits()
    );
    let nul = vec!["a\0b".to_owned()];
    assert!(!Start { args: &nul, ..good }.fits());
}

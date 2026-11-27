use super::*;

#[tokio::test]
async fn exact_bytes_without_a_successful_producer_never_finish_export() {
    for succeed in [false, true] {
        let (caller, broker) = UnixStream::pair().unwrap();
        let (_, mut sending) = broker.into_split();
        let child = producer(succeed);
        let result = child.export(&mut sending, 3).await;
        assert_eq!(result.is_ok(), succeed);
        drop(sending);

        let mut input = BufReader::new(caller);
        assert_eq!(
            read(&mut input).await.unwrap()["value"]["stream"],
            socket::EXPORT
        );
        let mut bytes = [0; 3];
        input.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes, b"abc");
        if succeed {
            assert_eq!(read(&mut input).await.unwrap()["value"]["exported"], true);
        }
        assert_eq!(input.read(&mut [0; 1]).await.unwrap(), 0);
    }
}

fn producer(succeed: bool) -> Worker {
    let status = if succeed { "0" } else { "7" };
    let mut child = tokio::process::Command::new("sh")
        .args(["-c", "printf abc; exit \"$1\"", "producer", status])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let input = child.stdin.take().unwrap();
    let output = BufReader::new(child.stdout.take().unwrap());
    Worker {
        child,
        input,
        output,
    }
}

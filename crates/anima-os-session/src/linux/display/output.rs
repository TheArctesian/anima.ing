use super::super::{Error, Result, command::run};
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub(super) const ROTATIONS: [&str; 4] = ["normal", "left", "right", "inverted"];
#[derive(Serialize, Deserialize)]
pub(super) struct Output {
    pub(super) name: String,
    pub(super) rotation: String,
    pub(super) panel: Option<String>,
}

pub(super) fn outputs(text: &str) -> Vec<Output> {
    let mut found = Vec::new();
    let mut current = None;
    for line in text.lines() {
        if line.starts_with(char::is_whitespace) {
            if let Some(index) = current
                && let Some((_, panel)) = line.split_once("panel orientation:")
            {
                let output: &mut Output = &mut found[index];
                output.panel = Some(panel.trim().into());
            }
            continue;
        }
        current = None;
        let words: Vec<_> = line.split_whitespace().collect();
        if words.get(1) != Some(&"connected") {
            continue;
        }
        let tail = line.split('(').next().unwrap_or("");
        if !tail
            .split_whitespace()
            .any(|part| part.contains('x') && part.contains('+'))
        {
            continue;
        }
        let rotation = tail
            .split_whitespace()
            .find(|word| ROTATIONS.contains(word))
            .unwrap_or("normal");
        found.push(Output {
            name: words[0].into(),
            rotation: rotation.into(),
            panel: None,
        });
        current = Some(found.len() - 1);
    }
    found
}

pub(in crate::linux) async fn list() -> Result<Value> {
    Ok(serde_json::to_value(outputs(
        &run("xrandr", &["--prop"], None, 5).await?,
    ))?)
}

pub(super) async fn rotate(output: &str, rotation: &str) -> Result<()> {
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        rotate_panel(output, rotation),
    )
    .await
    .map_err(|_| Error::Refused("The display did not respond. The preview will revert."))?
}

async fn rotate_panel(output: &str, rotation: &str) -> Result<()> {
    if !ROTATIONS.contains(&rotation) {
        return Err(Error::Refused("Invalid display orientation."));
    }
    run(
        "xrandr",
        &["--output", output, "--rotate", rotation],
        None,
        5,
    )
    .await?;
    let active = outputs(&run("xrandr", &["--prop"], None, 5).await?);
    if active.len() != 1 {
        return Ok(());
    }
    let matrix = match rotation {
        "left" => "0 -1 1 1 0 0 0 0 1",
        "right" => "0 1 0 -1 0 1 0 0 1",
        "inverted" => "-1 0 1 0 -1 1 0 0 1",
        _ => "1 0 0 0 1 0 0 0 1",
    };
    for id in run("xinput", &["--list", "--id-only"], None, 5)
        .await?
        .split_whitespace()
        .take(64)
    {
        if !id.chars().all(|letter| letter.is_ascii_digit()) {
            continue;
        }
        let properties = run("xinput", &["--list-props", id], None, 5).await?;
        let node = properties
            .lines()
            .find(|line| line.contains("Device Node"))
            .and_then(|line| line.split('"').nth(1));
        let Some(node) = node else {
            continue;
        };
        if run(
            "udevadm",
            &["info", "--query=property", "--name", node],
            None,
            5,
        )
        .await?
        .lines()
        .any(|line| line == "ID_INPUT_TOUCHSCREEN=1")
        {
            let mut args = vec!["--set-prop", id, "Coordinate Transformation Matrix"];
            args.extend(matrix.split_whitespace());
            run("xinput", &args, None, 5).await?;
        }
    }
    Ok(())
}

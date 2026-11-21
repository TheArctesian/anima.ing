use super::{Error, Result, keep, outputs, preview, read, revert, run};
use serde_json::json;

pub(in crate::linux) async fn cli(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("display-revert") => {
            let token = args
                .get(1)
                .map(|value| value.parse())
                .transpose()
                .map_err(|_| Error::Refused("Invalid preview identity."))?;
            revert(token).await
        }
        Some("display-save") => save_current(argument(args, 1)?).await,
        Some("display-set") => {
            preview(json!({"output":argument(args, 1)?,"rotation":argument(args, 2)?})).await?;
            Ok(())
        }
        Some("display-keep") => {
            let preview = read()?.ok_or(Error::Refused("The preview expired. Apply it again."))?;
            if preview.output != argument(args, 1)? {
                return Err(Error::Refused("The display preview changed."));
            }
            keep(preview.id).await?;
            Ok(())
        }
        _ => Err(Error::Refused("Unsupported display command.")),
    }
}

fn argument(args: &[String], index: usize) -> Result<&str> {
    args.get(index)
        .map(String::as_str)
        .ok_or(Error::Refused("Choose a display and orientation."))
}

pub(in crate::linux) async fn print_list() -> Result<()> {
    for output in outputs(&run("xrandr", &["--prop"], None, 5).await?) {
        println!("{}\t{}", output.name, output.rotation);
    }
    Ok(())
}

async fn save_current(name: &str) -> Result<()> {
    let output = outputs(&run("xrandr", &["--prop"], None, 5).await?)
        .into_iter()
        .find(|output| output.name == name)
        .ok_or(Error::Refused("Choose a connected display."))?;
    let token = preview(json!({"output":output.name,"rotation":output.rotation})).await?;
    keep(token).await?;
    Ok(())
}

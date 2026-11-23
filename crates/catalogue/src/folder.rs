//! The one part of the crate with I/O.

use std::{fs, io, path::Path};

use crate::{
    Recipe,
    fields::{COMPOSE, ENVIRONMENT},
};

const ENTRY: &str = "entry.json";

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("the folder of the catalogue cannot be read")]
    Folder(#[source] io::Error),
    #[error("a file of a recipe cannot be read")]
    File(#[source] io::Error),
}

fn text(path: &Path) -> Result<Option<String>, ReadError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(ReadError::File(error)),
    }
}

/// Each recipe below `root`, one for each folder, in the order of their names.
///
/// # Errors
/// [`ReadError`] when `root` or a file that is there cannot be read.
pub fn read(root: &Path) -> Result<Vec<Recipe>, ReadError> {
    let mut recipes = Vec::new();
    for found in fs::read_dir(root).map_err(ReadError::Folder)? {
        let found = found.map_err(ReadError::Folder)?;
        if !found.file_type().map_err(ReadError::Folder)?.is_dir() {
            continue;
        }
        let path = found.path();
        recipes.push(Recipe {
            folder: found.file_name().to_string_lossy().into_owned(),
            entry: text(&path.join(ENTRY))?,
            compose: text(&path.join(COMPOSE))?,
            environment: text(&path.join(ENVIRONMENT))?,
        });
    }
    recipes.sort_by(|left, right| left.folder.cmp(&right.folder));
    Ok(recipes)
}

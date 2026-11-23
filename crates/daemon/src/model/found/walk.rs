//! The walk of the folders of Section 10.2: the found models of each tool
//! (`contracts/compute/model-work.md` Sections 2.3 and 10.2). It reads the names, the sizes, the
//! Ollama manifests and the Hugging Face links; it reads no model file and writes nothing.

use std::collections::BTreeMap;

use daemon_core::model_work::found::{
    FOLDER_DEPTH, FoundFile, FoundModel, MAX_FOUND, Root, Tool, gguf_models, huggingface,
    is_shown_name, ollama,
};

use super::open::Folder;

/// The most locations that one level of a walk gives.
const MAX_LOCATIONS: usize = 4096;

/// The tool and the folder of Section 10.2 that the models of one walk have.
struct Base<'a> {
    tool: Tool,
    root: &'a str,
}

impl Base<'_> {
    fn model(&self, location: &[&str], name: String, files: Group) -> FoundModel {
        FoundModel {
            tool: self.tool,
            location: location.join("/"),
            name,
            root: self.root.to_owned(),
            files: files.files,
            usable: files.usable,
        }
    }
}

/// The files of one model; only GGUF is usable (MW-P10).
struct Group {
    files: Vec<FoundFile>,
    usable: bool,
}

/// The found models of each folder that is there, at most [`MAX_FOUND`].
#[must_use]
pub fn walk(roots: &[Root]) -> Vec<FoundModel> {
    let mut found = Vec::new();
    for root in roots {
        let (Some(folder), Some(text)) = (Folder::root(&root.path), root.path.to_str()) else {
            continue;
        };
        let base = Base {
            tool: root.tool,
            root: text,
        };
        let models = match root.tool {
            Tool::Ollama => ollama_models(&folder, &base),
            Tool::Huggingface => huggingface_models(&folder, &base),
            Tool::LmStudio => gguf_below(&folder, &base, 2, 2),
            Tool::Folder => gguf_below(&folder, &base, 0, FOLDER_DEPTH - 1),
        };
        found.extend(
            models
                .into_iter()
                .filter(|model| is_shown_name(&model.name)),
        );
        if found.len() >= MAX_FOUND {
            found.truncate(MAX_FOUND);
            break;
        }
    }
    found
}

/// The locations of the folders `levels` levels below `folder`.
fn folders_at(
    folder: &Folder,
    levels: usize,
    prefix: &mut Vec<String>,
    out: &mut Vec<Vec<String>>,
) {
    if levels == 0 {
        out.push(prefix.clone());
        return;
    }
    for name in folder.names() {
        if out.len() >= MAX_LOCATIONS {
            return;
        }
        if let Some(below) = folder.folder(&name) {
            prefix.push(name);
            folders_at(&below, levels - 1, prefix, out);
            prefix.pop();
        }
    }
}

fn parts(location: &[String]) -> Vec<&str> {
    location.iter().map(String::as_str).collect()
}

/// A manifest below `manifests/` names a model layer in `blobs/` of the size that it says.
fn ollama_models(folder: &Folder, base: &Base) -> Vec<FoundModel> {
    let (Some(manifests), Some(blobs)) = (folder.folder("manifests"), folder.folder("blobs"))
    else {
        return Vec::new();
    };
    let mut models = Vec::new();
    let mut locations = Vec::new();
    folders_at(
        &manifests,
        ollama::LEVELS - 1,
        &mut Vec::new(),
        &mut locations,
    );
    for location in &locations {
        let Some(model) = manifests.below(location.iter().map(String::as_str)) else {
            continue;
        };
        for tag in model.names() {
            let found = model
                .small_file(&tag, ollama::MAX_MANIFEST)
                .and_then(|manifest| ollama::layers(&manifest))
                .and_then(|layers| ollama_files(&blobs, &layers));
            if let (Some(files), [host, namespace, name]) = (found, parts(location).as_slice()) {
                let place = [*host, *namespace, *name, tag.as_str()];
                let group = Group {
                    files,
                    usable: true,
                };
                models.push(base.model(&place, ollama::shown_name(place), group));
            }
        }
    }
    models
}

fn ollama_files(blobs: &Folder, layers: &ollama::Layers) -> Option<Vec<FoundFile>> {
    std::iter::once(&layers.model)
        .chain(&layers.projector)
        .map(|blob| {
            let name = blob.file_name();
            (blobs.file_size(&name)? == blob.size_bytes).then(|| FoundFile {
                location: vec!["blobs".to_owned(), name],
                size_bytes: blob.size_bytes,
                named_sha256: Some(blob.sha256.clone()),
            })
        })
        .collect()
}

/// Each `models--<organisation>--<repository>/snapshots/<commit>/` of the cache.
fn huggingface_models(folder: &Folder, base: &Base) -> Vec<FoundModel> {
    let mut models = Vec::new();
    for repository in folder.names() {
        let Some((organisation, name)) = huggingface::repository(&repository) else {
            continue;
        };
        let Some(place) = folder.folder(&repository) else {
            continue;
        };
        let (Some(snapshots), blobs) = (place.folder("snapshots"), place.folder("blobs")) else {
            continue;
        };
        for commit in snapshots.names() {
            let Some(snapshot) = snapshots
                .folder(&commit)
                .filter(|_| huggingface::is_commit(&commit))
            else {
                continue;
            };
            let files = snapshot_files(folder, &snapshot, blobs.as_ref(), [&repository, &commit]);
            for (first, files) in grouped(&files) {
                let shown = huggingface::shown_name(organisation, name, first);
                models.push(base.model(&[&repository, &commit, first], shown, files));
            }
        }
    }
    models
}

/// The files of a snapshot: each a link to `../../blobs/<name>`, or a regular file.
fn snapshot_files(
    cache: &Folder,
    snapshot: &Folder,
    blobs: Option<&Folder>,
    [repository, commit]: [&str; 2],
) -> BTreeMap<String, FoundFile> {
    let mut files = BTreeMap::new();
    for name in snapshot.names() {
        let file = match snapshot.link(&name) {
            Some(text) => huggingface::blob_of_link(&text).and_then(|blob| {
                let (location, size_bytes) = blob_file(cache, blobs?, repository, blob)?;
                Some(FoundFile {
                    location,
                    size_bytes,
                    named_sha256: huggingface::blob_sha256(blob).map(str::to_owned),
                })
            }),
            None => snapshot.file_size(&name).map(|size_bytes| FoundFile {
                location: [repository, "snapshots", commit, &name]
                    .map(str::to_owned)
                    .to_vec(),
                size_bytes,
                named_sha256: None,
            }),
        };
        if let Some(file) = file {
            files.insert(name, file);
        }
    }
    files
}

/// The location below the cache and the size of the blob `blob` of `repository`: the regular file
/// `<repository>/blobs/<blob>`, or the file of the shared store that its link names.
fn blob_file(
    cache: &Folder,
    blobs: &Folder,
    repository: &str,
    blob: &str,
) -> Option<(Vec<String>, u64)> {
    let Some(text) = blobs.link(blob) else {
        let location = [repository, "blobs", blob].map(str::to_owned).to_vec();
        return Some((location, blobs.file_size(blob)?));
    };
    let (shard, stored) = huggingface::stored_of_link(&text)?;
    let size = cache.below(["blobs", shard])?.file_size(stored)?;
    Some((["blobs", shard, stored].map(str::to_owned).to_vec(), size))
}

/// The GGUF models among `files` by [`gguf_models`], each by the name of its first file; the
/// safetensors files of a folder are one model that is not usable (MW-P10).
fn grouped(files: &BTreeMap<String, FoundFile>) -> Vec<(&str, Group)> {
    let names: Vec<&str> = files.keys().map(String::as_str).collect();
    let group = |names: &[&str], usable| Group {
        files: names.iter().map(|name| files[*name].clone()).collect(),
        usable,
    };
    let mut models: Vec<(&str, Group)> = gguf_models(&names)
        .into_iter()
        .map(|model| (model[0], group(&model, true)))
        .collect();
    let tensors: Vec<&str> = names
        .iter()
        .copied()
        .filter(|name| name.to_ascii_lowercase().ends_with(".safetensors"))
        .collect();
    if let Some(first) = tensors.first() {
        models.push((first, group(&tensors, false)));
    }
    models
}

/// The GGUF models in the folders from `from` to `to` levels below `folder` (LM Studio:
/// `<publisher>/<repository>/<file>`; a folder of `modelFolders`: at most 3 levels).
fn gguf_below(folder: &Folder, base: &Base, from: usize, to: usize) -> Vec<FoundModel> {
    let mut models = Vec::new();
    for levels in from..=to {
        let mut locations = Vec::new();
        folders_at(folder, levels, &mut Vec::new(), &mut locations);
        for location in &locations {
            let below = if location.is_empty() {
                None
            } else {
                folder.below(location.iter().map(String::as_str))
            };
            let Some(place) = below.as_ref().or(location.is_empty().then_some(folder)) else {
                continue;
            };
            let files: BTreeMap<String, FoundFile> = place
                .names()
                .into_iter()
                .filter_map(|name| {
                    let size_bytes = place.file_size(&name)?;
                    let mut file_location = location.clone();
                    file_location.push(name.clone());
                    let file = FoundFile {
                        location: file_location,
                        size_bytes,
                        named_sha256: None,
                    };
                    Some((name, file))
                })
                .collect();
            for (first, files) in grouped(&files) {
                let mut place = parts(location);
                place.push(first);
                models.push(base.model(&place, place.join("/"), files));
            }
        }
    }
    models
}

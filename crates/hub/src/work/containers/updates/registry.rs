//! The name of an image as a place at a public registry (HOST-021). Pure.
//! A repository or a tag reaches a URL only when each character is one a registry permits, so no text of a device can change the host or the path of a request.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::work::containers) enum Registry {
    DockerHub,
    GitHub,
    Quay,
    Microsoft,
}

impl Registry {
    fn of(host: &str) -> Option<Self> {
        match host {
            "docker.io" | "index.docker.io" | "registry-1.docker.io" => Some(Self::DockerHub),
            "ghcr.io" => Some(Self::GitHub),
            "quay.io" => Some(Self::Quay),
            "mcr.microsoft.com" => Some(Self::Microsoft),
            _ => None,
        }
    }

    pub(super) const fn host(self) -> &'static str {
        match self {
            Self::DockerHub => "registry-1.docker.io",
            Self::GitHub => "ghcr.io",
            Self::Quay => "quay.io",
            Self::Microsoft => "mcr.microsoft.com",
        }
    }

    pub(super) const fn token(self) -> Option<(&'static str, &'static str, &'static str)> {
        match self {
            Self::DockerHub => Some(("auth.docker.io", "/token", "registry.docker.io")),
            Self::GitHub => Some(("ghcr.io", "/token", "ghcr.io")),
            Self::Quay | Self::Microsoft => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(in crate::work::containers) struct Reference {
    pub(in crate::work::containers) registry: Registry,
    /// With `library/` for an official image of Docker Hub.
    pub(in crate::work::containers) repository: String,
    pub(in crate::work::containers) tag: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::work::containers) enum Named {
    Tag(Reference),
    /// The name holds a digest: the container asked for that image and no newer one.
    Pinned,
    OtherRegistry,
    NoName,
}

const LONGEST_REPOSITORY: usize = 255;
const LONGEST_TAG: usize = 128;

fn repository_ok(text: &str) -> bool {
    let part_ok = |part: &str| {
        !part.is_empty()
            && part.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
            })
            && part
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_alphanumeric())
    };
    text.len() <= LONGEST_REPOSITORY && text.split('/').all(part_ok)
}

fn tag_ok(text: &str) -> bool {
    let first_ok = text
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    first_ok
        && text.len() <= LONGEST_TAG
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

/// What Docker shows when a container has no name for its image.
fn is_identifier(image: &str) -> bool {
    let digits = image.strip_prefix("sha256:");
    let hex = |text: &str| text.bytes().all(|byte| byte.is_ascii_hexdigit());
    digits.is_some() || (image.len() >= 12 && hex(image))
}

/// Reads an image name as Docker does: `[host[:port]/]repository[:tag][@digest]`.
pub(in crate::work::containers) fn named(image: &str) -> Named {
    if image.contains('@') {
        return Named::Pinned;
    }
    if is_identifier(image) {
        return Named::NoName;
    }
    let (host, rest) = match image.split_once('/') {
        Some((first, rest))
            if first.contains('.') || first.contains(':') || first == "localhost" =>
        {
            (first, rest)
        }
        _ => ("docker.io", image),
    };
    let Some(registry) = Registry::of(host) else {
        return Named::OtherRegistry;
    };
    let last = rest.rsplit('/').next().unwrap_or_default();
    let (repository, tag) = match last.rsplit_once(':') {
        Some((_, tag)) => (&rest[..rest.len() - tag.len() - 1], tag),
        None => (rest, "latest"),
    };
    if !repository_ok(repository) || !tag_ok(tag) {
        return Named::NoName;
    }
    let official = registry == Registry::DockerHub && !repository.contains('/');
    Named::Tag(Reference {
        registry,
        repository: if official {
            format!("library/{repository}")
        } else {
            repository.to_owned()
        },
        tag: tag.to_owned(),
    })
}

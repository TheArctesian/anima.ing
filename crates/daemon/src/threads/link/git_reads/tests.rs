use super::*;

fn hello(roots: Option<Vec<GitReadRoot>>) -> Event {
    Event::Hello {
        version: crate::threads::wire::VERSION,
        threads: Vec::new(),
        build: None,
        git_read_roots: roots,
    }
}

fn root(generation: u64) -> GitReadRoot {
    GitReadRoot {
        thread: uuid::Uuid::from_u128(1).to_string(),
        process: crate::threads::tree::identity::Identity {
            pid: 41,
            device: 1,
            inode: generation,
        },
        generation,
    }
}

#[test]
fn tc7_w3_older_holder_never_enrolls_a_root() {
    let holder = GitReads::new(Process::open(std::process::id()));
    holder.observe(&hello(None));
    holder.observe(&Event::GitReadRoot {
        root: root(1),
        enrolled: true,
    });
    assert!(LiveRoots::of(&holder).is_none());
}

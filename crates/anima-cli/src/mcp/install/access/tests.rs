//! R-442, plan Section 12.7.

use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _, symlink};

use super::{Groups, Listing, Member, Part, Writer, chain, creatable, program_writer, replaceable};

const USER: u32 = 501;

fn folder(owner: u32, group: u32, mode: u32) -> Part {
    Part {
        owner,
        group,
        mode,
        folder: true,
        link: false,
    }
}

fn groups(members: &[(&str, Option<u32>)]) -> Groups {
    let members: Vec<Member> = members
        .iter()
        .map(|(name, uid)| Member {
            name: (*name).to_owned(),
            uid: *uid,
        })
        .collect();
    Groups::new(move |group| Listing {
        name: format!("group{group}"),
        members: Some(members.clone()),
    })
}

fn unread() -> Groups {
    Groups::new(|group| Listing {
        name: format!("group{group}"),
        members: None,
    })
}

#[test]
fn r_exactgroup_a_group_of_only_the_person_and_root_can_write_the_folder() {
    // /opt/homebrew/bin as Homebrew makes it: the person, the group admin, mode 0775.
    let only_us = groups(&[("root", Some(0)), ("me", Some(USER)), ("me", Some(USER))]);
    let homebrew = folder(USER, 80, 0o40775);
    assert_eq!(program_writer(homebrew, USER, true, &only_us), None);
    assert_eq!(program_writer(homebrew, USER, false, &only_us), None);
    assert_eq!(
        program_writer(folder(USER, USER, 0o40775), USER, false, &only_us),
        None
    );
    assert_eq!(program_writer(homebrew, USER, true, &groups(&[])), None);
}

#[test]
fn r_exactgroup_a_group_with_a_different_member_is_refused_and_named() {
    let homebrew = folder(USER, 80, 0o40775);
    let shared = groups(&[("root", Some(0)), ("me", Some(USER)), ("alice", Some(502))]);
    assert_eq!(
        program_writer(homebrew, USER, true, &shared),
        Some(Writer::Member {
            group: "group80".to_owned(),
            member: "alice".to_owned()
        })
    );
    let stale = groups(&[("root", Some(0)), ("gone", None)]);
    assert!(matches!(
        program_writer(homebrew, USER, true, &stale),
        Some(Writer::Member { member, .. }) if member == "gone"
    ));
    assert_eq!(
        program_writer(folder(USER, 80, 0o40755), USER, true, &shared),
        None
    );
}

#[test]
fn r_exactgroup_a_folder_that_each_user_can_write_is_refused() {
    let only_us = groups(&[("root", Some(0))]);
    assert_eq!(
        program_writer(folder(USER, 80, 0o40777), USER, true, &only_us),
        Some(Writer::Everyone)
    );
    assert_eq!(
        program_writer(folder(0, 80, 0o40777), USER, true, &only_us),
        Some(Writer::Everyone)
    );
    // The sticky bit (as /tmp) lets only the owner of an entry replace it.
    assert_eq!(
        program_writer(folder(0, 0, 0o41777), USER, false, &only_us),
        None
    );
}

#[test]
fn mcp_035_a_new_entry_can_be_made_in_a_folder_with_the_sticky_bit_that_each_user_can_write() {
    let home = tempfile::tempdir().expect("a temporary folder");
    let user = fs::metadata(home.path()).expect("owner").uid();
    let sticky = home.path().join("sticky");
    fs::create_dir(&sticky).expect("folder");
    fs::set_permissions(&sticky, fs::Permissions::from_mode(0o1777)).expect("mode");
    let only_us = groups(&[]);
    assert_eq!(replaceable(&sticky, user, &only_us), None);
    assert!(matches!(
        creatable(&sticky, user, &only_us),
        Some(others) if others.writer == Writer::Everyone
    ));
    fs::set_permissions(&sticky, fs::Permissions::from_mode(0o1755)).expect("mode");
    assert_eq!(creatable(&sticky, user, &only_us), None);
}

#[test]
fn r_exactgroup_a_group_whose_members_cannot_be_read_is_refused() {
    assert_eq!(
        program_writer(folder(USER, 80, 0o40775), USER, true, &unread()),
        Some(Writer::Unread {
            group: "group80".to_owned()
        })
    );
}

#[test]
fn mcp_033_on_macos_a_folder_of_root_that_the_group_admin_can_write_is_fixed() {
    // `/Applications` as macOS makes it: root, group admin, mode 0775. A member of admin can
    // become root, so the members are not read.
    let applications = folder(0, 80, 0o40775);
    assert_eq!(program_writer(applications, USER, true, &unread()), None);
    assert!(program_writer(applications, USER, false, &unread()).is_some());
    assert!(matches!(
        program_writer(folder(502, 80, 0o40775), USER, true, &unread()),
        Some(Writer::Owner(_))
    ));
}

#[test]
fn mcp_033_each_link_of_a_chain_and_the_folder_that_holds_it_is_checked() {
    let home = tempfile::tempdir().expect("a temporary folder");
    let user = fs::metadata(home.path()).expect("owner").uid();
    let only_us = groups(&[]);
    for name in ["first", "middle", "last"] {
        fs::create_dir(home.path().join(name)).expect("folder");
    }
    let file = home.path().join("last/program");
    fs::write(&file, "").expect("file");
    symlink(
        home.path().join("middle/program"),
        home.path().join("first/program"),
    )
    .expect("link");
    symlink("../last/program", home.path().join("middle/program")).expect("link");
    let start = home.path().join("first/program");
    // On macOS the chain also holds `/var`, a link to `/private/var`.
    let resolved = chain(&start).expect("it resolves");
    assert_eq!(
        resolved.last(),
        Some(&fs::canonicalize(&file).expect("real"))
    );
    for link in ["first/program", "middle/program"] {
        let real = fs::canonicalize(home.path()).expect("real").join(link);
        assert!(resolved.contains(&real), "{link} is not in {resolved:?}");
    }
    assert_eq!(replaceable(&start, user, &only_us), None);
    let middle = home.path().join("middle");
    fs::set_permissions(&middle, fs::Permissions::from_mode(0o777)).expect("mode");
    let others = replaceable(&start, user, &only_us).expect("refused");
    assert_eq!(
        fs::canonicalize(&others.path).expect("real"),
        fs::canonicalize(&middle).expect("real")
    );
    assert_eq!(others.writer, Writer::Everyone);
    symlink(home.path().join("loop"), home.path().join("loop")).expect("link");
    assert!(matches!(
        replaceable(&home.path().join("loop"), user, &only_us),
        Some(others) if others.writer == Writer::Unknown
    ));
}

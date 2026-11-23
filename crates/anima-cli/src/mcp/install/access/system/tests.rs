//! How the account databases of Linux and macOS are read (R-NEW-exactgroup): each source of a
//! member, and each text that cannot be trusted counts as unread.

use std::fs;
use std::os::unix::fs::MetadataExt as _;

use super::linux::{entries, local_only, passwd};
use super::macos::{local_search, primary, record, resolve};
use super::{listing, number};
use crate::mcp::install::access::Member;

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn r_exactgroup_getent_gives_the_members_of_each_entry_and_each_account_whose_own_group_it_is() {
    let groups = "root:x:0:\nwheel:x:10:me\nshared:x:1000:me,alice\nagain:x:1000:bob\n";
    assert_eq!(entries(groups, 1000), Some(names(&["me", "alice", "bob"])));
    assert_eq!(entries(groups, 0), Some(Vec::new()));
    assert_eq!(entries("shared:x:1000\n", 1000), None);
    let text = "root:x:0:0:root:/root:/bin/bash\nme:x:1000:1000::/home/me:/bin/sh\nalice:x:1001:1000:Alice:/home/alice:/bin/sh\n";
    let member = |name: &str, uid| Member {
        name: name.to_owned(),
        uid: Some(uid),
    };
    assert_eq!(
        passwd(text, 1000),
        Some(vec![member("me", 1000), member("alice", 1001)])
    );
    assert_eq!(passwd(text, 7), Some(Vec::new()));
    // A line that is not an account: the list cannot be trusted.
    assert_eq!(passwd("me:x:1000\n", 1000), None);
    assert_eq!(passwd("me:x:one:1000::/:/bin/sh\n", 1000), None);
}

#[test]
fn r_exactgroup_only_sources_that_list_each_account_and_group_are_read() {
    assert!(local_only(
        "# comment\npasswd:  files systemd\ngroup: files [SUCCESS=merge] systemd # local\n"
    ));
    assert!(local_only("passwd: files\ngroup: files\n"));
    // A directory service need not list each account, nor each member of a group.
    assert!(!local_only("passwd: files sss\ngroup: files\n"));
    assert!(!local_only("passwd: files\ngroup: files winbind\n"));
    assert!(!local_only("passwd: compat\ngroup: compat\n"));
    // A line that is not there, that names no source, or that comes again (the last one counts).
    assert!(!local_only("passwd: files\n"));
    assert!(!local_only("passwd:\ngroup: files\n"));
    assert!(!local_only(
        "passwd: files\ngroup: files\npasswd: files sss\n"
    ));
    // glibc also reads a line with a blank after the name of the database in place of a colon.
    assert!(!local_only(
        "passwd: files\ngroup: files\npasswd files sss\n"
    ));
    assert!(!local_only("passwd: files\ngroup: files\npasswd\u{b}sss\n"));
    // The groups given at login.
    assert!(local_only(
        "passwd: files\ngroup: files\ninitgroups: files\n"
    ));
    assert!(!local_only(
        "passwd: files\ngroup: files\ninitgroups: sss files\n"
    ));
}

#[test]
fn r_exactgroup_on_macos_only_a_search_path_of_this_computer_alone_is_read() {
    assert!(local_search("SearchPath: /Local/Default\n"));
    assert!(local_search("SearchPath:\n /Local/Default\n /BSD/Local\n"));
    assert!(!local_search(
        "SearchPath:\n /Local/Default\n /Active Directory/CORP/All Domains\n"
    ));
    assert!(!local_search("SearchPath:\n"));
    assert!(!local_search(""));
}

#[test]
fn r_exactgroup_dscl_gives_the_accounts_whose_own_group_it_is() {
    let users = "_www                      70\nme                        20\nalice                     20\nnobody\n";
    assert_eq!(primary(users, 20), Some(names(&["me", "alice"])));
    assert_eq!(primary("me 20 21\n", 20), None);
    // `nobody` has the group -2, which is 4294967294; it does not stop the list.
    assert_eq!(
        primary("nobody -2\nme 20\n", 4_294_967_294),
        Some(names(&["nobody"]))
    );
    assert_eq!(number("-2"), Some(4_294_967_294));
    assert_eq!(number("4294967295"), Some(u32::MAX));
    assert_eq!(number("4294967296"), None);
    assert_eq!(number("-2147483649"), None);
}

#[test]
fn r_exactgroup_a_dscl_record_names_each_member_and_refuses_what_it_cannot_name() {
    let admin =
        "GroupMembers: FFFF-0 AAAA-501 BBBB-502\nGroupMembership: root me me\nPrimaryGroupID: 80\n";
    let (listed, identifiers) = record(admin, 80).expect("a record");
    assert_eq!(listed, names(&["root", "me", "me"]));
    // Each member by identifier is named, also one that the list of names leaves out.
    let accounts = "root FFFF-0\nme AAAA-501\nbob BBBB-502\n";
    assert_eq!(
        resolve(&identifiers, accounts),
        Some(names(&["root", "me", "bob"]))
    );
    assert_eq!(resolve(&identifiers, "root FFFF-0\nme AAAA-501\n"), None);
    // The record of a different group of the same name.
    assert_eq!(record(admin, 20), None);
    // A value on the next line, after a space.
    assert_eq!(
        record("GroupMembership:\n root me\nPrimaryGroupID: 20\n", 20),
        Some((names(&["root", "me"]), Vec::new()))
    );
    // No list of members, as `everyone`, whose members the system calculates; a nested group.
    assert_eq!(record("PrimaryGroupID: 12\n", 12), None);
    assert_eq!(record("", 12), None);
    assert_eq!(
        record(
            "GroupMembership: root\nNestedGroups: ABCDEFAB-CDEF-ABCD-EFAB-CDEF00000050\nPrimaryGroupID: 20\n",
            20
        ),
        None
    );
}

#[test]
fn r_exactgroup_the_system_lists_the_person_in_the_group_of_a_new_file() {
    // A computer that also takes accounts from a directory service reads no group.
    if cfg!(target_os = "linux")
        && !fs::read_to_string("/etc/nsswitch.conf").map_or(true, |text| local_only(&text))
    {
        return;
    }
    // A new file belongs to the user of the process, in the group of that user or of its folder.
    let file = tempfile::NamedTempFile::new().expect("a file");
    let metadata = fs::metadata(file.path()).expect("metadata");
    let user = metadata.uid();
    let found = listing(metadata.gid());
    let members = found.members.expect("the members can be read");
    assert!(
        members.iter().any(|member| member.uid == Some(user)),
        "{} lists {members:?}",
        found.name
    );
    // Each user is in `everyone` of macOS, which lists no member.
    if cfg!(target_os = "macos") {
        assert_eq!(listing(12).members, None);
    }
}

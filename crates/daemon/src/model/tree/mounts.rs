//! A cgroup namespace stops a move of a process out of its root only when cgroup v2 is mounted
//! with `nsdelegate` (R-330).

#[must_use]
pub fn cgroup_has_nsdelegate(mountinfo: &str) -> bool {
    mountinfo.lines().any(|line| {
        let Some((before, after)) = line.split_once(" - ") else {
            return false;
        };
        let point = before.split_whitespace().nth(4);
        let mut fields = after.split_whitespace();
        let kind = fields.next();
        let options = fields.nth(1).unwrap_or_default();
        let mount_options = before.split_whitespace().nth(5).unwrap_or_default();
        point == Some("/sys/fs/cgroup")
            && kind == Some("cgroup2")
            && options
                .split(',')
                .chain(mount_options.split(','))
                .any(|option| option == "nsdelegate")
    })
}

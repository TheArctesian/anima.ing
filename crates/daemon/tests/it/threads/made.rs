//! The made programs of the tests of the runner: Claude Code, Codex and a login shell.

/// A made Claude Code: it names its session, asks to run a command, and after the answer starts
/// a child in a process group of its own, writes the child's ID to `child.pid`, and ends its turn.
pub(super) const CLAUDE: &str = r#"#!/usr/bin/perl
$| = 1;
my $sid = "none";
for my $i (0 .. $#ARGV) { $sid = $ARGV[$i + 1] if $ARGV[$i] eq "--session-id"; }
print qq({"type":"system","subtype":"init","session_id":"$sid","model":"made","permissionMode":"bypassPermissions"}\n);
while (my $line = <STDIN>) {
    if ($line =~ /"type":"user"/) {
        print qq({"type":"system","subtype":"session_state_changed","state":"running"}\n);
        print qq({"type":"control_request","request_id":"r1","request":{"subtype":"can_use_tool","tool_name":"Bash","input":{"command":"sleep 300"},"permission_suggestions":[],"tool_use_id":"t1"}}\n);
    } elsif ($line =~ /"control_response"/) {
        my $pid = fork();
        if ($pid == 0) { setpgrp(0, 0); exec("/bin/sleep", "300"); }
        open(my $file, ">", "child.pid"); print $file $pid; close($file);
        print qq({"type":"result","subtype":"success","is_error":false,"result":"done","terminal_reason":"completed"}\n);
        print qq({"type":"system","subtype":"session_state_changed","state":"idle"}\n);
    }
}
"#;

/// A made login shell: it is asked as a login and interactive shell, sets the names of a
/// profile (one of them with the value of a secret of Anima), and runs the script it is given.
pub(super) const SHELL: &str = "#!/bin/sh\n[ \"$1\" = -l ] && [ \"$2\" = -i ] || exit 3\nexport LOGIN_ONLY=yes COPY=s3cret-value ANIMA_LEAK=x\nexec /bin/sh -c \"$4\"\n";

/// A made `codex app-server` that answers `initialize` and `model/list` only, so that no test
/// reaches a real copy in the usual folders.
pub(super) const CODEX: &str = r#"#!/usr/bin/perl
$| = 1;
while (my $line = <STDIN>) {
    my ($id) = $line =~ /"id":(\d+)/;
    if ($line =~ /"method":"initialize"/) {
        print qq({"id":$id,"result":{}}\n);
    } elsif ($line =~ /"method":"model\/list"/) {
        print qq({"id":$id,"result":{"data":[{"model":"made-codex","displayName":"Made","supportedReasoningEfforts":[{"reasoningEffort":"low"}],"defaultReasoningEffort":"low"}]}}\n);
    }
}
"#;

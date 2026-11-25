#!/usr/bin/perl
use strict;
use warnings;
use Cwd 'getcwd';
use File::Basename 'basename';
use IO::Socket::UNIX;
use JSON::PP 'encode_json', 'decode_json';
use Socket 'SOCK_STREAM';

$| = 1;
my $session = "made";
for my $i (0 .. $#ARGV) {
    $session = $ARGV[$i+1] if $ARGV[$i] eq "--session-id";
}
print encode_json({type=>"system", subtype=>"init", session_id=>$session, model=>"made", permissionMode=>"default"}), "\n";
while (my $line = <STDIN>) {
    next unless $line =~ /"type":"user"/;
    print qq({"type":"system","subtype":"session_state_changed","state":"running"}\n);
    if ($line =~ /Read Git summaries/) {
        my $child = fork();
        die "cannot fork" unless defined $child;
        if ($child == 0) {
            probe();
            exit 0;
        }
        waitpid($child, 0);
    }
    print qq({"type":"result","subtype":"success","is_error":false,"result":"done","terminal_reason":"completed"}\n);
    print qq({"type":"system","subtype":"session_state_changed","state":"idle"}\n);
}
sub probe {
    my $name = basename(getcwd());
    my $other = $name eq "work" ? "other" : "work";
    my @requests = (
        [$name, {operation=>"status"}],
        [$name, {operation=>"checkpoints", limit=>1}],
        [$name, {operation=>"changes", scope=>{scope=>"uncommitted"}}],
        [$other, {operation=>"status"}],
    );
    my @outcomes;
    for my $i (0 .. $#requests) {
        my $socket = IO::Socket::UNIX->new(Type=>SOCK_STREAM, Peer=>"$ENV{HOME}/daemon/threads/runner.sock")
            or die "cannot reach fixture runner";
        my ($repository, $read) = @{$requests[$i]};
        print $socket encode_json({id=>$i+1, call=>"git_read", repository=>$repository, read=>$read}), "\n";
        my $reply = <$socket>;
        die "runner did not answer Git read" unless defined $reply;
        push @outcomes, decode_json($reply);
    }
    my $path = "$ENV{HOME}/reads-$name.json";
    open my $output, ">", "$path.new" or die "cannot write fixture output";
    print $output encode_json(\@outcomes);
    close $output;
    rename "$path.new", $path or die "cannot publish fixture output";
}

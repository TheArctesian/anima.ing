#!/usr/bin/perl
use strict;
use warnings;
use IO::Socket::UNIX;
use Socket 'SOCK_STREAM';
$| = 1;

my $path = $ARGV[0];
my $socket;
while (my $command = <STDIN>) {
    if ($command eq "connect\n") {
        $socket = IO::Socket::UNIX->new(Type => SOCK_STREAM, Peer => $path) or die "connect";
        $socket->autoflush(1);
        print "connected\n";
    } elsif ($command eq "half-close\n") {
        shutdown($socket, 1) or die "half close";
        print "half-closed\n";
    } elsif ($command eq "read\n") {
        while (my $line = <$socket>) { print $line; }
        close $socket;
        print "eof\n";
    } else {
        print $socket $command or die "write";
    }
}

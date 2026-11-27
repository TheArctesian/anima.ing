set -eu
test "$#" -eq 4
test "$1" = enrol
test "$2" = -
test "$3" = --hub
case "$4" in
  unix:/var/lib/anima-daemon/link.sock|https://hub.example.invalid/) ;;
  *) exit 1 ;;
esac

cd /var/lib/anima-daemon
cat > code
printf 'enrol\n' >> fixture-attempts
if [ -e fixture-hold ]; then
  touch fixture-entered
  while [ -e fixture-hold ]; do sleep 0.05; done
fi
test ! -e fixture-reject
echo '{"synthetic":true}' > daemon.json

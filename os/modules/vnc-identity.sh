umask 0077
cd /persist/vnc
exec 9>.lock
flock 9

validate() {
  test -d "$1"
  test ! -L "$1"
  for name in cert.pem key.pem; do
    test -f "$1/$name"
    test ! -L "$1/$name"
    test "$(stat -c '%u:%a' "$1/$name")" = '0:600'
  done
  certificate_public=$(openssl x509 -in "$1/cert.pem" -pubkey -noout)
  key_public=$(openssl pkey -in "$1/key.pem" -pubout)
  test "$certificate_public" = "$key_public"
}

if test -e identity || test -L identity; then
  validate identity
  exit 0
fi

# Only unpublished material in this root-owned directory may be regenerated.
rm -rf -- .pending
mkdir -m 0700 .pending
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes \
  -days 3650 -subj /CN=AnimaOS-VNC \
  -keyout .pending/key.pem -out .pending/cert.pem
chmod 0600 .pending/key.pem .pending/cert.pem
validate .pending
sync -f .pending/key.pem
sync -f .pending/cert.pem
mv -T .pending identity
sync -f .

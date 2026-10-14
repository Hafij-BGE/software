#!/bin/sh
# Makes the signing key for Craft Suite's Android app, once. Android installs an update over the
# app only when both are signed with the same key, so every build must use this one.
#
# Usage: sh packaging/make-android-key.sh [folder]     (default folder: ~/craft-suite-android-key)
#
# Needs only openssl. It writes, in the folder (readable only by you):
#   craft-suite.p12        the key (PKCS#12): keep it, with its password, somewhere safe
#   password.txt           its password (random)
#   craft-suite.p12.b64    the key as text, for the ANDROID_KEYSTORE_BASE64 secret
# and says which two GitHub secrets to add.
set -eu

DIR="${1:-$HOME/craft-suite-android-key}"
if [ -e "$DIR/craft-suite.p12" ]; then
  echo "There is already a key in $DIR; not replacing it (a new key would stop updates)." >&2
  exit 1
fi
command -v openssl >/dev/null 2>&1 || { echo "openssl is needed: sudo apt install openssl" >&2; exit 1; }

umask 077
mkdir -p "$DIR"
openssl rand -hex 24 > "$DIR/password.txt"
PASS="$(cat "$DIR/password.txt")"
openssl req -x509 -newkey rsa:3072 -sha256 -nodes -days 36500 -subj "/CN=Craft Suite" \
  -keyout "$DIR/key.pem" -out "$DIR/cert.pem" 2>/dev/null
openssl pkcs12 -export -name craft-suite -inkey "$DIR/key.pem" -in "$DIR/cert.pem" \
  -out "$DIR/craft-suite.p12" -passout "pass:$PASS"
rm -f "$DIR/key.pem" "$DIR/cert.pem"
base64 -w0 "$DIR/craft-suite.p12" > "$DIR/craft-suite.p12.b64" 2>/dev/null ||
  base64 "$DIR/craft-suite.p12" | tr -d '\n' > "$DIR/craft-suite.p12.b64"

cat <<EOF

Made the key in $DIR.

Now add two secrets on GitHub: the repository › Settings › Secrets and variables › Actions ›
New repository secret.

  1. Name: ANDROID_KEYSTORE_BASE64
     Secret: everything in $DIR/craft-suite.p12.b64
     (to copy it: xclip -selection clipboard < "$DIR/craft-suite.p12.b64", or open the file in a
     text editor, select all, copy)

  2. Name: ANDROID_KEYSTORE_PASSWORD
     Secret: the line in $DIR/password.txt

Keep $DIR (or a copy of craft-suite.p12 and password.txt) safe and private: with a new key,
installed apps can't be updated. Don't put it in the repository or send it to anyone.
EOF

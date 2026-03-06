#!/bin/bash
set -euo pipefail

CERT_DIR=$(dirname "$(realpath "$0")")
SERVER_KEY="$CERT_DIR/key.pem"
SERVER_CSR="$CERT_DIR/server.csr"
SERVER_CERT="$CERT_DIR/server.crt"
EXT_FILE="$CERT_DIR/server.ext"

openssl ecparam -name prime256v1 -genkey -noout -out "$SERVER_KEY"

openssl req -new -key "$SERVER_KEY" -out "$SERVER_CSR" \
    -subj "/CN=ocypode-server"

cat > "$EXT_FILE" <<EOF
basicConstraints = critical, CA:FALSE
keyUsage = critical, digitalSignature
extendedKeyUsage = serverAuth, clientAuth
subjectAltName = DNS:localhost, IP:127.0.0.1
EOF

openssl x509 -req \
    -in "$SERVER_CSR" \
    -signkey "$SERVER_KEY" \
    -out "$SERVER_CERT" \
    -days 3650 \
    -extfile "$EXT_FILE"

rm "$EXT_FILE"

echo "Generated:"
echo "  cert: $SERVER_CERT"
echo "  key:  $SERVER_KEY"

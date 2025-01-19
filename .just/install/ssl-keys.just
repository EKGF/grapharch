
repo_root := env_var_or_default('repo_root', invocation_directory())

export SSL_DIR := join(repo_root, ".ssl")
export SSL_CA_CERT_FILE := join(SSL_DIR, "cacert.pem")
export SSL_KEY_FILE := join(SSL_DIR, "localhost.key")
export SSL_CERT_FILE := join(SSL_DIR, "localhost.crt")

ssl-info:
  #!/usr/bin/env bash
  echo "SSL_DIR: ${SSL_DIR}"
  echo "SSL_CA_CERT_FILE: ${SSL_CA_CERT_FILE}"
  echo "SSL_KEY_FILE: ${SSL_KEY_FILE}"
  echo "SSL_CERT_FILE: ${SSL_CERT_FILE}"

_ssl_dir_create:
  #!/usr/bin/env bash
  test -d "${SSL_DIR}" || mkdir -p "${SSL_DIR}"

_ssl_keys: _ssl_key_file _ssl_cert_file

_ssl_key_file:
  #!/usr/bin/env bash
  test -f "${SSL_KEY_FILE}" || just _ssl-keys-generate

_ssl_cert_file:
  #!/usr/bin/env bash
  test -f "${SSL_CERT_FILE}" || just _ssl-keys-generate

_ssl-keys-generate: _ssl_dir_create
  #!/usr/bin/env bash

  if [[ ! -f "${SSL_KEY_FILE}" ]]; then
    echo "Generating SSL certificates in ${SSL_DIR}..."
    openssl req -x509 -newkey rsa:4096 \
      -keyout "${SSL_KEY_FILE}" \
      -out "${SSL_CERT_FILE}" \
      -days 365 \
      -nodes \
      -subj "/CN=localhost"
  fi

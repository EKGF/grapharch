#!/usr/bin/env bash
#
# Serve the docs site locally
#
# Usage:
#   ./serve.sh
#
# This script will serve the docs site locally using Jekyll.
# It will use the configuration file _config_dev.yml for the local environment.
#
# The script assumes that the Jekyll and Bundler are installed and available in the PATH.
#
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
export GEM_HOME="${SCRIPT_DIR}/.gems"
mkdir -p "${GEM_HOME}" > /dev/null 2>&1 || true
export PATH="${SCRIPT_DIR}/.gems/bin:$PATH"
export SSL_CERT_FILE="${SCRIPT_DIR}/.ssl/cacert.pem"

if [[ ! -f "${SCRIPT_DIR}/.ssl/localhost.key" ]]; then
  echo "Generating SSL certificates..."
  mkdir -p "${SCRIPT_DIR}/.ssl"
  openssl req -x509 -newkey rsa:4096 -keyout "${SCRIPT_DIR}/.ssl/localhost.key" -out "${SCRIPT_DIR}/.ssl/localhost.crt" -days 365 -nodes -subj "/CN=localhost"
fi

cd "${SCRIPT_DIR}"

# openssl x509 -in "${SCRIPT_DIR}/.ssl/localhost.crt" -text -noout

echo "GEM_HOME: ${GEM_HOME}"
bundle install --gemfile="${SCRIPT_DIR}/Gemfile"
bundle update --gemfile="${SCRIPT_DIR}/Gemfile"

# bundle info jekyll

rm -rf "${SCRIPT_DIR}/_site"

# echo "Checking GitHub Pages Health..."
# bundle exec github-pages health-check
# exit 1

export DISABLE_WHITELIST=true

bundle exec jekyll serve \
  --trace \
  --host localhost \
  --port 4000 \
  --ssl-key ".ssl/localhost.key" \
  --ssl-cert ".ssl/localhost.crt" \
  --incremental \
  --livereload \
  --config "_config.yml,_config_dev.yml"

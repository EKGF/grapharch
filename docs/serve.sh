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

export SSL_CERT_FILE=.ssl/cacert.pem

openssl x509 -in .ssl/localhost.crt -text -noout

bundle exec jekyll serve \
  --trace \
  --host localhost \
  --port 4000 \
  --ssl-key .ssl/localhost.key \
  --ssl-cert .ssl/localhost.crt \
  --incremental \
  --livereload \
  --config "_config.yml,_config_dev.yml"

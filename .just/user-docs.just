import "install/ruby-gem.just"
import "install/ssl-keys.just"

repo_root := env_var_or_default('repo_root', invocation_directory())
user_docs_dir := env_var_or_default('user_docs_dir', join(repo_root, "docs"))
export ruby_version := env_var_or_default('ruby_version', "3.3.6")

export SSL_DIR := join(repo_root, ".ssl")
export SSL_CA_CERT_FILE := join(SSL_DIR, "cacert.pem")
export SSL_KEY_FILE := join(SSL_DIR, "localhost.key")
export SSL_CERT_FILE := join(SSL_DIR, "localhost.crt")

user-docs-build: _user_docs_install_gem_file
  #!/usr/bin/env bash
  cd {{user_docs_dir}}
  bundle exec jekyll build

user-docs-clean:
  #!/usr/bin/env bash -x
  cd {{user_docs_dir}}
  test -d _site && rm -rf _site
  test -d .jekyll-cache && rm -rf .jekyll-cache
  test -d .sass-cache && rm -rf .sass-cache
  test -f .jekyll-metadata && rm -f .jekyll-metadata
  test -d .jekyll-cache && rm -rf .jekyll-cache
  test -d .jekyll-metadata && rm -rf .jekyll-metadata
  test -f Gemfile.lock && rm -f Gemfile.lock

user-docs-serve: _user_docs_install_gem_file _ssl_keys
  #!/usr/bin/env bash
  cd {{user_docs_dir}}
  export DISABLE_WHITELIST=true
  bundle exec jekyll serve \
    --trace \
    --host localhost \
    --port 4000 \
    --ssl-key "${SSL_KEY_FILE}" \
    --ssl-cert "${SSL_CERT_FILE}" \
    --incremental \
    --livereload \
    --config "_config.yml,_config_dev.yml"

user-docs-serve-no-ssl: _user_docs_install_gem_file
  #!/usr/bin/env bash
  cd {{user_docs_dir}}
  export DISABLE_WHITELIST=true
  bundle exec jekyll serve \
    --trace \
    --host localhost \
    --port 4000 \
    --incremental \
    --livereload \
    --config "_config.yml,_config_dev.yml"

_user_docs_install_gem_file: gem-info _install-jekyll
  #!/usr/bin/env bash
  cd {{user_docs_dir}}
  echo "{{ruby_version}}" > .ruby-version
  bundle install --gemfile=./Gemfile

[no-exit-message]
user-docs-bundle *args: _install-gem-bundle
  #!/usr/bin/env bash
  cd {{user_docs_dir}}
  if [[ "{{args}}" == "install" ]]; then
    bundle --gemfile=./Gemfile {{args}}
  else
    bundle {{args}}
  fi

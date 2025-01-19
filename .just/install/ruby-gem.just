import "sentinel.just"
import "ruby.just"

path-separator := if os_family() == "windows" { ";" } else { ":" }

existing_gem_home := env_var_or_default('GEM_HOME', "")
gem-path := join(env_var('HOME'), ".gems") + if existing_gem_home == "" {
  ""
} else {
  path-separator + existing_gem_home
}
repo_root := env_var_or_default('repo_root', invocation_directory())

export GEM_HOME := join(repo_root, ".gems")
export GEM_PATH := GEM_HOME + path-separator + gem-path

gem-info: ruby-info
  #!/usr/bin/env bash
  echo "---------------------------- gem"
  if test -f {{sentinel_gem_bundle_installed}}; then
    echo "  binary: $(which gem)"
    echo " version: $(gem --version)"
    echo "GEM_HOME: ${GEM_HOME}"
    echo "GEM_PATH: ${GEM_PATH}"
  else
    echo "gem is not installed"
  fi
  echo "---------------------------- bundle"
  if test -f {{sentinel_gem_bundle_installed}}; then
    echo "  binary: $(which bundle)"
    echo " version: $(bundle --version)"
  else
    echo "bundle is not installed"
  fi

_gem_home_create:
  #!/usr/bin/env bash
  test -d {{GEM_HOME}} || mkdir -p {{GEM_HOME}}

_install-gem-bundle: _install-ruby && _upgrade-gem
  #!/usr/bin/env bash
  if test -f {{sentinel_gem_bundle_installed}}; then
    echo "gem bundle is already installed"
  else
    echo "installing gem bundler"
    cd {{user_docs_dir}}
    gem install bundler
    bundle install
    bundle update --bundler
    touch {{sentinel_gem_bundle_installed}}
  fi

_upgrade-gem:
  #!/usr/bin/env bash
  if test -f {{sentinel_gem_upgraded}}; then
    echo "gem is already upgraded"
  else
    echo "upgrading gem"
    cd {{user_docs_dir}}
    gem update --system
    gem update
    touch {{sentinel_gem_upgraded}}
  fi

_install-jekyll: _install-gem-bundle
  #!/usr/bin/env bash
  if test -f {{sentinel_jekyll_installed}}; then
    echo "jekyll is already installed"
  else
    echo "installing jekyll with gem"
    cd {{user_docs_dir}}
    gem install jekyll
    touch {{sentinel_jekyll_installed}}
  fi

[no-exit-message]
gem *args: _install-gem-bundle
  #!/usr/bin/env bash
  gem {{args}}

[no-exit-message]
bundle *args: _install-gem-bundle
  #!/usr/bin/env bash
  bundle {{args}}

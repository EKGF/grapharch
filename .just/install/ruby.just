import "sentinel.just"

set script-interpreter := ['/bin/sh', '-eu']

path-separator := if os_family() == "windows" { ";" } else { ":" }

export HOMEBREW_PREFIX := env_var_or_default('HOMEBREW_PREFIX', shell('brew --prefix'))
export ruby_version := env_var_or_default('ruby_version', "3.3.6")
export RUBY_BIN_DIR := join(env_var('HOMEBREW_PREFIX'), "opt", "ruby", "bin")
export PATH := RUBY_BIN_DIR + path-separator + join(env_var('HOME'), ".rubies", ruby_version, "bin") + path-separator + env_var('PATH')

ruby-info:
  #!/usr/bin/env bash
  echo "---------------------------- ruby-install"
  echo " binary: $(which ruby-install)"
  echo "version: $(ruby-install --version)"
  echo "---------------------------- ruby"
  echo "required version: {{ruby_version}}"
  echo "  actual version: $(ruby --version)"
  echo "          binary: $(which ruby)"
  echo "    RUBY_BIN_DIR: {{RUBY_BIN_DIR}}"
  echo "            PATH: {{PATH}}"
  echo "---------------------------- gem"
  echo "  binary: $(which gem)"
  echo " version: $(gem --version)"
  echo "---------------------------- bundler"
  echo "  binary: $(which bundler)"
  echo " version: $(bundle --version)"

# install ruby-install
[script]
_install-ruby-install: _mkdir_sentinel_dir
  if test -f {{sentinel_ruby_install_installed}}; then
    echo "ruby-install is already installed"
  else
    echo "installing ruby-install"
    brew install ruby-install
    touch {{sentinel_ruby_install_installed}}
  fi

# install ruby
[script]
_install-ruby: _install-ruby-install
  if test -f {{sentinel_ruby_installed}}; then
    echo "ruby is already installed"
  else
    echo "installing ruby {{ruby_version}}"
    cd {{user_docs_dir}}
    ruby-install ruby {{ruby_version}}
    touch {{sentinel_ruby_installed}}
  fi


#!/usr/bin/env bash
#
# Install the tools needed to build the docs site.
#
# This script has only been tested on macOS.
# It may work on Linux or Windows WSL but you have to have
# Homebrew installed first.
#
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
export GEM_HOME="${SCRIPT_DIR}/.gems"
mkdir -p "${GEM_HOME}" > /dev/null 2>&1 || true
export PATH="${SCRIPT_DIR}/.gems/bin:$PATH"

if ! command -v brew &> /dev/null; then
  echo "Installing Homebrew..."
  /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
fi

brew install ruby-install

ruby-install ruby 3.3.6

bundle install
bundle update


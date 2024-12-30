#!/usr/bin/env bash
#
# Install the tools needed to build the docs site.
#
# This script has only been tested on macOS.
# It may work on Linux or Windows WSL but you have to have
# Homebrew installed first.
#
brew install ruby-install

ruby-install ruby 3.3.6

bundle install
bundle update


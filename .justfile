set dotenv-load := true
set dotenv-required := true
set fallback := false
set allow-duplicate-recipes := true
set allow-duplicate-variables := true
set unstable := true
set export := true
set positional-arguments := true

home_dir := env_var('HOME')
repo_root := `git rev-parse --show-toplevel`
export ruby_version := "3.3.6"
export user_docs_dir := join(repo_root, "docs")

import "./.just/user-docs.just"

help:
    @just --list

info:
  @echo "CPU architecture: {{ arch() }}"
  @echo "Operating system type: {{ os_family() }}"
  @echo "Operating system: {{ os() }}"
  @echo "Home directory: {{ home_directory() }}"
  @echo "home_dir: {{home_dir}}"
  @echo "repo_root: {{repo_root}}"
  @echo "user_docs_dir: {{user_docs_dir}}"

GraphArch-build:
  cargo +nightly build

# notify update in keybase
notify m="":
	keybase chat send --topic-type "chat" --channel <channel> <team> "upd(<repo>): {{m}}"

docs: user-docs-build user-docs-serve-no-ssl

build: GraphArch-build user-docs-build

rebuild:
  cargo clean
  just build

check:
  cargo +nightly check --all-features --verbose
  cargo +nightly clippy --all-features --message-format=json
  cargo +nightly fmt -- --check

test:
  cargo +nightly test --all-features
  # cargo +nightly test --all-features -- --nocapture

run *args:
  cargo +nightly run -- {{args}}

lint:
  cargo +nightly clippy --all-features --message-format=json
  cargo +nightly fmt -- --check

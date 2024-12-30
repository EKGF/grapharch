---
title: "Git Setup"
layout: default
permalink: /contribute/git-setup
---

# Git setup

## Set the right email address before committing

```shell
git config user.email "youremail@yourdomain.com"
git config user.name "Your Name"
```

## Try to avoid merge commits

```shell
git config --local branch.autosetuprebase always
git config --local merge.ff only
```

## Configure signed git commits

Assuming you have [Homebrew](https://brew.sh) installed:

```shell
git config --local commit.gpgsign true
brew update
brew upgrade
brew uninstall gpg
brew install gpg2
git config --local gpg.program /usr/local/bin/gpg
git config --local commit.gpgsign true
```

Then after gpg has been installed, create a key:

```shell
gpg --full-generate-key
```

Then show all keys:

```shell
gpg --list-keys
```

Then copy the long number of your key into the clipboard
and register it as follows:

```shell
git config --local user.signingkey <Key from your list>
```

Then tell GitHub know what your key is by first exporting it
(copy the output &mdash; with START and END line included &mdash; into the clipboard):

```shell
gpg --armor --export <Key from your list>
```

Then add this key to GitHub at GPG keys: <https://github.com/settings/keys>

Your commits should now work and be signed.

Check [GitHub docs](https://docs.github.com/en/authentication/managing-commit-signature-verification/signing-commits) for other options.

## References

- [How to prevent merge conflicts](https://dev.to/github/how-to-prevent-merge-conflicts-or-at-least-have-less-of-them-109p)
- [Git rebase for preventing merge commits](https://jenchan.biz/blog/git-rebase-for-preventing-merge-commits)

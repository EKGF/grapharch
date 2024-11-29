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

## References

- [How to prevent merge conflicts](https://dev.to/github/how-to-prevent-merge-conflicts-or-at-least-have-less-of-them-109p)
- [Git rebase for preventing merge commits](https://jenchan.biz/blog/git-rebase-for-preventing-merge-commits)

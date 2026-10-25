#!/usr/bin/env python3
"""Resolve the exact introduced commit range without revisiting published history."""
import os
from pathlib import Path
import re
import subprocess


def git(*arguments):
    return subprocess.check_output(["git", *arguments], text=True).strip()


def revision(value):
    if not re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", value):
        raise ValueError("Expected a complete commit SHA")
    git("rev-parse", "--verify", f"{value}^{{commit}}")
    return value


def commit_range(environment):
    head = revision(environment["GRAPHARCH_HEAD"])
    event = environment["GRAPHARCH_EVENT"]
    if event == "pull_request":
        base = revision(environment["GRAPHARCH_BASE"])
    elif event == "push":
        before = environment["GRAPHARCH_BEFORE"]
        if before in ("0" * 40, "0" * 64):
            branch = "refs/remotes/origin/" + environment["GRAPHARCH_DEFAULT_BRANCH"]
            git("check-ref-format", branch)
            base = revision(git("merge-base", head, branch))
        else:
            base = revision(before)
    else:
        raise ValueError(f"Unsupported commit-check event: {event}")
    return f"{base}..{head}"


def main():
    result = commit_range(os.environ)
    if output := os.environ.get("GITHUB_OUTPUT"):
        with Path(output).open("a", encoding="utf-8") as writer:
            writer.write(f"range={result}\n")
    print(result)


if __name__ == "__main__":
    main()

"""Git helpers for pytest-diff.

Extracted from plugin.py to keep the main module focused on pytest hooks.
"""

from __future__ import annotations


def get_git_commit_sha(rootdir: str) -> str | None:
    """Get the current HEAD commit SHA from git.

    Returns None if git is unavailable, not a repo, or any error occurs.
    """
    import subprocess

    try:
        result = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=rootdir,
            capture_output=True,
            text=True,
            timeout=5,
        )
        if result.returncode == 0:
            return result.stdout.strip()
    except (subprocess.TimeoutExpired, FileNotFoundError, OSError):
        pass
    return None


def check_baseline_staleness(baseline_commit: str, rootdir: str) -> str | None:
    """Check if the baseline commit is stale relative to current HEAD.

    Returns None if the baseline is current, or a warning message string.
    """
    import subprocess

    current_sha = get_git_commit_sha(rootdir)
    if current_sha is None:
        return None

    if baseline_commit == current_sha:
        return None

    short_baseline = baseline_commit[:10]
    short_head = current_sha[:10]

    try:
        result = subprocess.run(
            ["git", "merge-base", "--is-ancestor", baseline_commit, "HEAD"],
            cwd=rootdir,
            capture_output=True,
            timeout=5,
        )
        if result.returncode == 0:
            return (
                f"Baseline was built from commit {short_baseline}, "
                f"current HEAD is {short_head}. "
                f"Baseline is older but included in your history. "
                f"Test selection may not be optimal for newly merged code."
            )
    except (subprocess.TimeoutExpired, FileNotFoundError, OSError):
        pass

    return (
        f"Baseline is STALE: built from commit {short_baseline} "
        f"which is NOT in your current history (HEAD={short_head}). "
        f"Test selection may be unreliable. "
        f"Consider re-running: pytest --diff-baseline"
    )

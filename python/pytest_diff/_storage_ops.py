"""Remote storage operations for pytest-diff.

Extracted from plugin.py to keep the main module focused on pytest hooks.
"""

from __future__ import annotations

import logging
import tempfile
import time
from pathlib import Path
from typing import Any

logger = logging.getLogger("pytest_diff")


def init_storage(
    storage: Any,
    remote_url: str | None,
) -> Any:
    """Lazily initialize the remote storage backend.

    Returns the storage object (possibly newly created), or None.
    """
    if storage is not None or not remote_url:
        return storage
    try:
        from pytest_diff.storage import get_storage

        storage = get_storage(remote_url)
        if storage is None:
            logger.warning("⚠ pytest-diff: Unsupported remote URL scheme: %s", remote_url)
    except Exception as e:
        logger.warning("⚠ pytest-diff: Failed to initialize remote storage: %s", e)
    return storage


def download_and_import_baseline(
    storage: Any,
    remote_url: str | None,
    remote_key: str,
    db: Any,
    db_path: Path,
    rootdir: str,
    log: Any,
) -> Any:
    """Download remote baseline DB and import via ATTACH.

    *log* is a ``logging.Logger`` instance.

    Returns the (possibly newly created) storage object.
    """
    storage = init_storage(storage, remote_url)
    if storage is None:
        return storage

    dl_start = time.time()
    # Use NamedTemporaryFile for unique filename (avoids race conditions with xdist)
    with tempfile.NamedTemporaryFile(suffix=".db", delete=False) as tmp:
        tmp_path = Path(tmp.name)

    try:
        try:
            downloaded = storage.download(remote_key, tmp_path)
            if downloaded:
                log.debug("Downloaded remote baseline in %.3fs", time.time() - dl_start)
            else:
                log.debug("Remote baseline unchanged (cache hit)")
        except FileNotFoundError:
            log.debug("No remote baseline found — skipping import")
            return storage
        except Exception as e:
            logger.warning("⚠ pytest-diff: Failed to download remote baseline: %s", e)
            return storage

        if db is None:
            return storage
        try:
            import_start = time.time()
            count = db.import_baseline_from(str(tmp_path))
            log.debug(
                "Imported %s baseline fingerprints in %.3fs", count, time.time() - import_start
            )
            logger.info(
                "✓ pytest-diff: Imported %s baseline fingerprints from remote into %s",
                count,
                db_path,
            )

            baseline_commit = db.get_metadata("baseline_commit")
            if baseline_commit:
                from pytest_diff._git import check_baseline_staleness

                warning = check_baseline_staleness(baseline_commit, rootdir)
                if warning:
                    logger.warning("⚠ pytest-diff: %s", warning)
            else:
                log.debug("No baseline_commit metadata found — skipping staleness check")
        except Exception as e:
            logger.warning("⚠ pytest-diff: Failed to import remote baseline: %s", e)
    finally:
        # Clean up temp file
        tmp_path.unlink(missing_ok=True)

    return storage


def upload_baseline(
    storage: Any,
    remote_url: str | None,
    remote_key: str,
    db_path: Path,
    log: Any,
) -> Any:
    """Upload local baseline DB to remote storage.

    *log* is a ``logging.Logger`` instance.

    Returns the (possibly newly created) storage object.
    """
    storage = init_storage(storage, remote_url)
    if storage is None:
        return storage

    try:
        upload_start = time.time()
        storage.upload(db_path, remote_key)
        log.debug("Uploaded baseline in %.3fs", time.time() - upload_start)
        assert remote_url is not None
        url = remote_url.rstrip("/") + "/" + remote_key.lstrip("/")
        logger.info("✓ pytest-diff: Uploaded baseline to %s", url)
    except Exception as e:
        logger.warning("⚠ pytest-diff: Failed to upload baseline: %s", e)

    return storage

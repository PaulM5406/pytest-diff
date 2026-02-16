"""Amazon S3 storage backend (``s3://`` URLs).

Requires the ``boto3`` package (install with ``pip install pytest-difftest[s3]``).
Uses the standard AWS credential chain (env vars, ``~/.aws/credentials``,
IAM roles, etc.).

ETag-based conditional downloads: a ``.etag`` sidecar file is stored next to
the cached DB so we can skip re-downloading unchanged baselines.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

from pytest_difftest.storage.base import BaselineStorage, StorageAuthenticationError


class S3Storage(BaselineStorage):
    """Store/retrieve baseline DB on Amazon S3.

    URL format: ``s3://bucket-name/optional/prefix/``
    """

    def __init__(self, url: str) -> None:
        # Parse s3://bucket/prefix/
        without_scheme = url.removeprefix("s3://")
        parts = without_scheme.split("/", 1)
        self.bucket = parts[0]
        self.prefix = parts[1].rstrip("/") + "/" if len(parts) > 1 and parts[1] else ""
        self._client = None

    @property
    def client(self) -> Any:
        if self._client is None:
            try:
                import boto3
            except ImportError as exc:
                raise ImportError(
                    "boto3 is required for S3 storage. Install with: pip install pytest-difftest[s3]"
                ) from exc
            self._client = boto3.client("s3")
        return self._client

    def _s3_key(self, remote_key: str) -> str:
        return f"{self.prefix}{remote_key}"

    def _check_auth_error(self, exc: Exception, context: str) -> None:
        """Raise ``StorageAuthenticationError`` if *exc* is an AWS auth failure."""
        _AUTH_ERROR_CODES = frozenset(
            (
                "AccessDenied",
                "ExpiredToken",
                "InvalidAccessKeyId",
                "SignatureDoesNotMatch",
                "TokenRefreshRequired",
            )
        )
        response = getattr(exc, "response", None)
        if not isinstance(response, dict):
            return
        error_code = response.get("Error", {}).get("Code", "")
        http_code = response.get("ResponseMetadata", {}).get("HTTPStatusCode", 0)
        if error_code in _AUTH_ERROR_CODES or http_code in (401, 403):
            raise StorageAuthenticationError(f"S3 authentication failed {context}: {exc}") from exc

    def upload(self, local_path: Path, remote_key: str) -> None:
        s3_key = self._s3_key(remote_key)
        try:
            self.client.upload_file(str(local_path), self.bucket, s3_key)
        except Exception as exc:
            self._check_auth_error(exc, f"uploading to s3://{self.bucket}/{s3_key}")
            raise

    def download(self, remote_key: str, local_path: Path) -> bool:
        s3_key = self._s3_key(remote_key)
        etag_path = local_path.with_suffix(local_path.suffix + ".etag")

        # Read cached ETag if available
        cached_etag: str | None = None
        if etag_path.exists() and local_path.exists():
            cached_etag = etag_path.read_text().strip()

        # Conditional GET — skip download if ETag matches
        try:
            kwargs: dict = {"Bucket": self.bucket, "Key": s3_key}
            if cached_etag:
                kwargs["IfNoneMatch"] = cached_etag

            response = self.client.get_object(**kwargs)
        except self.client.exceptions.NoSuchKey:
            raise FileNotFoundError(f"Remote baseline not found: s3://{self.bucket}/{s3_key}")
        except Exception as exc:
            # boto3 wraps 304 Not Modified as a ClientError
            resp = getattr(exc, "response", None)
            if isinstance(resp, dict):
                code = resp.get("Error", {}).get("Code", "")
                http = resp.get("ResponseMetadata", {}).get("HTTPStatusCode", 0)
                if code == "304" or http == 304:
                    return False
            self._check_auth_error(exc, f"for s3://{self.bucket}/{s3_key}")
            raise

        # Write file and save ETag
        local_path.parent.mkdir(parents=True, exist_ok=True)
        with open(local_path, "wb") as f:
            for chunk in response["Body"].iter_chunks():
                f.write(chunk)

        new_etag = response.get("ETag", "")
        if new_etag:
            etag_path.write_text(new_etag)

        return True

    def list_baselines(self, prefix: str = "") -> list[str]:
        """List all .db files under a prefix.

        Args:
            prefix: Additional prefix to append to the storage prefix.

        Returns:
            List of S3 keys (full paths) for all .db files found.

        Raises:
            StorageAuthenticationError: If S3 credentials are invalid or expired.
        """
        full_prefix = f"{self.prefix}{prefix}"
        keys: list[str] = []

        try:
            # Use paginator to handle >1000 objects
            paginator = self.client.get_paginator("list_objects_v2")
            for page in paginator.paginate(Bucket=self.bucket, Prefix=full_prefix):
                for obj in page.get("Contents", []):
                    key = obj["Key"]
                    if key.endswith(".db"):
                        keys.append(key)
        except Exception as exc:
            self._check_auth_error(exc, f"listing s3://{self.bucket}/{full_prefix}")
            raise

        return keys

    def download_all(self, local_dir: Path, prefix: str = "") -> list[Path]:
        """Download all .db files from the configured prefix to local_dir.

        Args:
            local_dir: Local directory to download files to.
            prefix: Additional prefix to append to the storage prefix.

        Returns:
            List of local file paths that were downloaded.
        """
        keys = self.list_baselines(prefix)
        downloaded: list[Path] = []

        for key in keys:
            # Use the filename from the key
            filename = Path(key).name
            local_path = local_dir / filename
            self.client.download_file(self.bucket, key, str(local_path))
            downloaded.append(local_path)

        return downloaded

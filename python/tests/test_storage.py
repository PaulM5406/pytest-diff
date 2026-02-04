"""Tests for local and S3 storage backends."""

from __future__ import annotations

from pathlib import Path

import pytest


class TestLocalStorage:
    """Tests for the local filesystem storage backend."""

    def test_upload_download_roundtrip(self, tmp_path: Path) -> None:
        from pytest_diff.storage.local import LocalStorage

        remote_dir = tmp_path / "remote"
        remote_dir.mkdir()
        storage = LocalStorage(f"file://{remote_dir}")

        # Create a local file and upload it
        local_file = tmp_path / "local.db"
        local_file.write_bytes(b"hello baseline")
        storage.upload(local_file, "baseline.db")

        # Download to a new location
        dest = tmp_path / "downloaded.db"
        downloaded = storage.download("baseline.db", dest)
        assert downloaded is True
        assert dest.read_bytes() == b"hello baseline"

    def test_download_not_found(self, tmp_path: Path) -> None:
        from pytest_diff.storage.local import LocalStorage

        remote_dir = tmp_path / "remote"
        remote_dir.mkdir()
        storage = LocalStorage(f"file://{remote_dir}")

        with pytest.raises(FileNotFoundError):
            storage.download("missing.db", tmp_path / "out.db")

    def test_download_cache_hit(self, tmp_path: Path) -> None:
        from pytest_diff.storage.local import LocalStorage

        remote_dir = tmp_path / "remote"
        remote_dir.mkdir()
        storage = LocalStorage(f"file://{remote_dir}")

        # Upload a file
        local_file = tmp_path / "local.db"
        local_file.write_bytes(b"data")
        storage.upload(local_file, "baseline.db")

        # First download: returns True
        dest = tmp_path / "downloaded.db"
        assert storage.download("baseline.db", dest) is True

        # Second download with same file already present: returns False (cache hit)
        assert storage.download("baseline.db", dest) is False


class TestS3Storage:
    """Tests for the S3 storage backend using moto."""

    @pytest.fixture(autouse=True)
    def _require_moto(self) -> None:
        pytest.importorskip("moto")

    @pytest.fixture()
    def s3_storage(self, tmp_path: Path):
        import boto3
        from moto import mock_aws  # type: ignore[import-not-found]

        from pytest_diff.storage.s3 import S3Storage

        with mock_aws():
            client = boto3.client("s3", region_name="us-east-1")
            client.create_bucket(Bucket="test-bucket")

            storage = S3Storage("s3://test-bucket/prefix/")
            # Override the lazily-created client with the mock one
            storage._client = client
            yield storage

    def test_upload_download_roundtrip(self, s3_storage, tmp_path: Path) -> None:
        local_file = tmp_path / "local.db"
        local_file.write_bytes(b"s3 baseline data")
        s3_storage.upload(local_file, "baseline.db")

        dest = tmp_path / "downloaded.db"
        downloaded = s3_storage.download("baseline.db", dest)
        assert downloaded is True
        assert dest.read_bytes() == b"s3 baseline data"

    def test_download_not_found(self, s3_storage, tmp_path: Path) -> None:
        with pytest.raises(FileNotFoundError):
            s3_storage.download("missing.db", tmp_path / "out.db")

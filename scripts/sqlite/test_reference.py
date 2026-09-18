"""Run after build-reference.sh; tests require the actual qualified artifacts."""
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import build_reference as reference


class ReferenceQualification(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.prefix = Path(os.environ.get("REDLINEDB_SQLITE_REFERENCE_PREFIX",
                                       reference.BASE / reference.VERSION))
        cls.receipt = json.loads((cls.prefix / "oracle-identity.json").read_text())

    def test_artifact_hashes_and_identity(self):
        self.assertEqual(self.receipt["source_id"], reference.SOURCE_ID)
        self.assertEqual(self.receipt["script_sha256"], reference.digest(Path(reference.__file__)))
        for path, expected in self.receipt["artifacts"].items():
            self.assertEqual(reference.digest(self.prefix / path), expected, path)

    def test_both_execution_paths(self):
        self.assertEqual(reference.probe(self.prefix, "extended")["cli_and_library"], "passed")

    def test_positive_fixture_rejected_by_both_engines_is_not_success(self):
        with patch.object(reference, "COMMON_SQL", "SELECT no_such_function();"):
            with self.assertRaises(subprocess.CalledProcessError):
                reference.probe(self.prefix, "extended")

    def test_wrong_output_fails(self):
        with patch.object(reference, "COMMON_EXPECTED", "wrong\n"):
            with self.assertRaisesRegex(RuntimeError, "execution mismatch"):
                reference.probe(self.prefix, "extended")

    def test_wrong_embedded_source_identity_fails(self):
        with patch.object(reference, "SOURCE_ID", "wrong source"):
            with self.assertRaisesRegex(RuntimeError, "source identity mismatch"):
                reference.probe(self.prefix, "extended")

    def test_cache_rejects_changed_identity_partial_manifest_and_tampering(self):
        with tempfile.TemporaryDirectory() as temp:
            prefix = Path(temp)
            receipt = dict(self.receipt)
            receipt["artifacts"] = {}
            for name in self.receipt["artifacts"]:
                file = prefix / name
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_text("fixture")
                receipt["artifacts"][name] = reference.digest(file)
            path = prefix / "oracle-identity.json"
            path.write_text(json.dumps(receipt))
            self.assertTrue(reference.cache_current(prefix, receipt["cache_key"]))
            self.assertFalse(reference.cache_current(prefix, "different compiler or flags"))
            (prefix / "include/sqlite3.h").write_text("tampered")
            self.assertFalse(reference.cache_current(prefix, receipt["cache_key"]))
            del receipt["artifacts"]["include/sqlite3.h"]
            path.write_text(json.dumps(receipt))
            self.assertFalse(reference.cache_current(prefix, receipt["cache_key"]))

    def test_download_digest_mismatch_fails_before_extraction(self):
        with tempfile.TemporaryDirectory() as temp, \
             patch.object(reference, "BASE", Path(temp)), \
             patch.dict(os.environ, {"REDLINEDB_SQLITE_REFERENCE_PREFIX": temp + "/install"}), \
             patch.object(reference.urllib.request, "urlopen", return_value=io.BytesIO(b"tampered")):
            with self.assertRaisesRegex(RuntimeError, "archive SHA3 mismatch"):
                reference.main()


if __name__ == "__main__":
    unittest.main()

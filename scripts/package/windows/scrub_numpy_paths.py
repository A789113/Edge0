"""Replace build-machine paths that prebuilt wheels baked into metadata files.

numpy's own wheel ships numpy/__config__.py holding the absolute paths of the
machine that built it. package.ps1 verifies that no packaged file contains this
machine's profile path, so that wheel fails the check on a runner whose username
happens to match. __config__ only feeds numpy.show_config(); no numeric routine
reads it, so rewriting the path strings is safe.
"""
import re
import sys
from pathlib import Path

# Any "...Users\<name>..." fragment, with either slash, optionally preceded by a
# drive letter. Keeps the rest of the file byte-for-byte intact.
PATTERN = re.compile(rb"(?:[A-Za-z]:)?[\\/]+[Uu]sers[\\/]+[^\"'\r\n\\/]+[\\/]?")


def main(root: str) -> int:
    changed = 0
    for path in sorted(Path(root).rglob("__config__.py")):
        data = path.read_bytes()
        fixed = PATTERN.sub(b"<build-machine>/", data)
        if fixed != data:
            path.write_bytes(fixed)
            print(f"scrubbed {path}")
            changed += 1
    print(f"scrub_numpy_paths: {changed} file(s) rewritten")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))

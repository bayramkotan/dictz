"""Only one dictz window: the instance lock is exclusive between processes."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

from dictz.core.instance import InstanceLock

CHILD = """
import sys
from dictz.core.instance import InstanceLock
print("yes" if InstanceLock(sys.argv[1]).acquire() else "no")
"""


def _other_process_gets_it(data: Path) -> bool:
    out = subprocess.run([sys.executable, "-c", CHILD, str(data)], capture_output=True, text=True,
                         cwd=Path(__file__).resolve().parents[1], check=True)
    return out.stdout.strip() == "yes"


def test_second_process_is_refused_until_the_first_ends(tmp_path) -> None:
    lock = InstanceLock(tmp_path)
    assert lock.acquire()
    assert not _other_process_gets_it(tmp_path)
    lock.release()
    assert _other_process_gets_it(tmp_path)

"""PyInstaller entry point.

dictz/__main__.py uses a relative import (`from .app import main`), which only
works when run as part of the package (`python -m dictz`). PyInstaller runs its
entry script as a top-level module, so it needs an absolute import instead.
"""
import multiprocessing

from dictz.app import main

if __name__ == "__main__":
    # dictionaries are read in worker processes; their start goes through here
    multiprocessing.freeze_support()
    raise SystemExit(main())

import sys

from . import __version__

if len(sys.argv) > 1 and sys.argv[1] in ("--version", "-V"):
    print(f"distro-builder {__version__}")
    sys.exit(0)

from .ui import main  # noqa: E402

sys.exit(main())

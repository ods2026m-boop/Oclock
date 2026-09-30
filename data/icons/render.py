#!/usr/bin/env python3
"""Render OClock's raster icon sizes from its vector artwork.

The SVG in ``hicolor/scalable`` is the artwork; every PNG in this set is a
straight scale of it, generated from that one file. Nothing is drawn here and
nothing is downloaded: the inputs are two files already in the repository, so a
render never needs a network, a package index or a source host.

Two ways to render, in order of preference:

1. librsvg through GObject Introspection, which is what rsvg-convert uses;
2. rsvg-convert itself, which is the same library behind a command line.

Neither being installed is not a failure. It means this machine cannot render,
the checked-in PNGs stand, and the script says so and exits zero — a machine
without a renderer should not stop a build that has nothing to render.

A renderer that *is* installed and then fails is a failure, and is reported as
one with a non-zero exit status. So is a renderer that exits zero without
writing the file, or writes a truncated one. Every size is checked on disk
before the script says it succeeded, so "success" cannot be printed for a
partial set — which is the failure a package build cannot detect for itself.
"""

from __future__ import annotations

import shutil
import subprocess
import sys
import warnings
from pathlib import Path

# The sizes the freedesktop icon theme specification names for the `apps`
# context, plus 512 because that is the size software stores an icon at.
SIZES = (16, 22, 24, 32, 48, 64, 128, 256, 512)

HERE = Path(__file__).resolve().parent
APP_ID = "org.odsos.OClock"
SCALABLE = HERE / "hicolor" / "scalable" / "apps" / f"{APP_ID}.svg"
THEMEDIR = HERE / "hicolor"

# A generous ceiling: these are flat shapes, and a machine that needs longer
# than this is not going to finish at all.
TIMEOUT = 120

PNG_MAGIC = b"\x89PNG\r\n\x1a\n"


class RenderUnavailable(Exception):
    """This renderer is not installed, so it cannot be tried."""


class RenderFailed(Exception):
    """A renderer ran and did not produce the file it was asked for."""


def render_with_rsvg_convert(size: int, destination: Path) -> None:
    """Renders one size with the rsvg-convert command line."""
    try:
        subprocess.run(
            [
                "rsvg-convert",
                "-w",
                str(size),
                "-h",
                str(size),
                "-o",
                str(destination),
                str(SCALABLE),
            ],
            check=True,
            capture_output=True,
            timeout=TIMEOUT,
        )
    except FileNotFoundError as error:
        raise RenderUnavailable("rsvg-convert is not installed") from error
    except subprocess.CalledProcessError as error:
        detail = error.stderr.decode(errors="replace").strip() or "no output"
        raise RenderFailed(f"rsvg-convert failed: {detail}") from error
    except subprocess.TimeoutExpired as error:
        raise RenderFailed(f"rsvg-convert timed out after {TIMEOUT}s") from error

    verify(destination, size)


def render_with_introspection(size: int, destination: Path) -> None:
    """Renders one size through librsvg's introspection bindings."""
    try:
        import cairo
        import gi

        gi.require_version("Rsvg", "2.0")
        from gi.repository import Rsvg
    except (ImportError, ValueError) as error:
        # No bindings: a renderer this machine does not have, rather than a
        # render that went wrong.
        raise RenderUnavailable(f"librsvg introspection is unavailable: {error}") from error

    try:
        handle = Rsvg.Handle.new_from_file(str(SCALABLE))
        surface = cairo.ImageSurface(cairo.FORMAT_ARGB32, size, size)
        # `render_cairo` is the only spelling of this that works across the
        # librsvg versions a desktop is likely to have — the newer
        # `render_document` changed its arguments twice — so its deprecation
        # notice is silenced rather than printed on every icon.
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", DeprecationWarning)
            handle.render_cairo(cairo.Context(surface))
        surface.write_to_png(str(destination))
    except Exception as error:  # noqa: BLE001 — reported, never swallowed
        raise RenderFailed(f"librsvg failed: {error}") from error

    verify(destination, size)


def verify(destination: Path, size: int) -> None:
    """Checks that the file on disk is a PNG of the size that was asked for.

    A renderer can exit zero having written nothing, or having written half a
    file. Either way the icon set is incomplete and a package would install it
    without complaint and then draw the wrong thing.
    """
    if not destination.exists():
        raise RenderFailed(f"{destination.name} was not written")

    data = destination.read_bytes()
    if len(data) < 24 or not data.startswith(PNG_MAGIC):
        raise RenderFailed(f"{destination.name} is not a PNG")

    width = int.from_bytes(data[16:20], "big")
    height = int.from_bytes(data[20:24], "big")
    if (width, height) != (size, size):
        raise RenderFailed(f"{destination.name} is {width}x{height}, not {size}x{size}")


def available_renderers() -> list[tuple[str, object]]:
    """The renderers this machine has, best first.

    Availability is a property of the machine and not an error; a renderer that
    is absent is simply not offered.
    """
    found: list[tuple[str, object]] = []

    reason = None
    try:
        import cairo
        import gi

        gi.require_version("Rsvg", "2.0")
        from gi.repository import Rsvg  # noqa: F401 — importing it is the probe

        found.append(("librsvg", render_with_introspection))
    except (ImportError, ValueError) as error:
        reason = error

    if shutil.which("rsvg-convert"):
        found.append(("rsvg-convert", render_with_rsvg_convert))
    elif reason is None:
        reason = RuntimeError("librsvg introspection did not load")

    return found


def render(size: int, destination: Path) -> str:
    """Renders one size with whichever renderer will do it, and returns its name.

    Each renderer in turn, so a machine with one working renderer and one broken
    one still produces the icon set. A size no renderer could produce is the
    failure this script exits non-zero for.
    """
    destination.parent.mkdir(parents=True, exist_ok=True)

    failures: list[str] = []
    renderers = available_renderers()
    for name, renderer in renderers:
        try:
            renderer(size, destination)  # type: ignore[operator]
        except RenderUnavailable as error:
            failures.append(f"{name}: {error}")
        except RenderFailed as error:
            failures.append(f"{name}: {error}")
        else:
            return name

    detail = "; ".join(failures) or "no renderer is installed"
    raise RenderFailed(detail)


def main() -> int:
    if not SCALABLE.exists():
        print(f"render: the artwork is missing: {SCALABLE}", file=sys.stderr)
        return 1

    if not available_renderers():
        print(
            "render: neither librsvg nor rsvg-convert is available, so the "
            "checked-in sizes stand; nothing was rendered"
        )
        return 0

    used = ""
    for size in SIZES:
        destination = THEMEDIR / f"{size}x{size}" / "apps" / f"{APP_ID}.png"
        try:
            used = render(size, destination)
        except RenderFailed as error:
            print(f"render: {size}px: {error}", file=sys.stderr)
            return 1
        print(f"render: {size}px")

    # Reached only once every size is on disk and has been verified, so this
    # line cannot be printed for a partial set.
    print(f"render: {len(SIZES)} sizes rendered and verified with {used}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
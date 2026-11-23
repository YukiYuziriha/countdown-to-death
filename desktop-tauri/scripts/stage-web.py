"""Stage the shared web UI for the Tauri bundle without a frontend build."""

from pathlib import Path
import shutil


WEB_FILES = (
    "ui-prototype.html",
    "prototype.css",
    "animated-texture.js",
    "reminder-schedule.js",
    "countdown-format.js",
    "app-host.js",
    "assets/PixelifySans.ttf",
    "assets/PixelifySans-OFL.txt",
    "assets/Silkscreen-Regular.ttf",
    "assets/Silkscreen-OFL.txt",
)


def main():
    desktop = Path(__file__).resolve().parents[1]
    root = desktop.parent
    dist = desktop / "dist"

    missing = [name for name in WEB_FILES if not (root / name).is_file()]
    if missing:
        raise SystemExit("Missing required web inputs:\n  " + "\n  ".join(missing))

    # Start clean so stale files cannot be embedded in the next bundle.
    if dist.exists():
        shutil.rmtree(dist)
    for name in WEB_FILES:
        destination = dist / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(root / name, destination)

    print(f"Staged {len(WEB_FILES)} web files in {dist}")


if __name__ == "__main__":
    main()

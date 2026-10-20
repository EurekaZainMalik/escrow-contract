#!/usr/bin/env python3
"""Fail if a Rust source file under the contract's src directory is not declared as a module."""
import glob
import os
import re
import sys

SRC = "contracts/milestone-escrow/src"
RUST_FILES = os.path.join(SRC, "**", "*.rs")
MODULE_DECLARATION = re.compile(
    r'^\s*(?:#\s*\[\s*path\s*=\s*"([^"]+)"\s*\]\s*)?'
    r"(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;",
    re.MULTILINE,
)


def module_source_dir(source_path: str) -> str:
    directory = os.path.dirname(source_path)
    if os.path.basename(source_path) in ("lib.rs", "main.rs", "mod.rs"):
        return directory
    return os.path.join(directory, os.path.splitext(os.path.basename(source_path))[0])


def main() -> int:
    source_files = {
        os.path.normpath(path)
        for path in glob.glob(RUST_FILES, recursive=True)
    }
    referenced = set()

    for source_path in source_files:
        with open(source_path, encoding="utf-8", errors="replace") as handle:
            source = handle.read()

        for explicit_path, module_name in MODULE_DECLARATION.findall(source):
            if explicit_path:
                target = os.path.join(os.path.dirname(source_path), explicit_path)
            else:
                base = module_source_dir(source_path)
                target = os.path.join(base, module_name + ".rs")
                # `mod name;` may also resolve to `name/mod.rs`.
                if not os.path.exists(target):
                    target = os.path.join(base, module_name, "mod.rs")
            referenced.add(os.path.normpath(target))

    orphans = sorted(
        os.path.relpath(path, ".")
        for path in source_files
        if os.path.normpath(path) != os.path.normpath(os.path.join(SRC, "lib.rs"))
        and path not in referenced
    )

    if not orphans:
        print("Rust source modules: no orphans")
        return 0

    print(f"Rust source modules: {len(orphans)} orphaned file(s)\n")
    for path in orphans:
        print(f"  {path}")
    print(
        "\nDeclare each file with `mod name;` or `#[path = \"...\"] mod name;`, "
        "or remove files that are no longer needed."
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())

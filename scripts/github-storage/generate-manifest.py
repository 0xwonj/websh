#!/usr/bin/env python3
"""
Generate manifest.json from content directory.

Supports:
- Frontmatter parsing (title, tags)
- Directory metadata via .meta.json
- Git timestamps (modified)
- Encrypted files via .keys sidecar files
"""

import json
import os
import re
import subprocess
from pathlib import Path
from typing import Any

# Configuration
CONTENT_DIR = "~"
OUTPUT_FILE = "manifest.json"
EXTENSIONS = {".md", ".pdf", ".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".link", ".enc"}


def parse_frontmatter(content: str) -> dict[str, Any]:
    """Parse YAML frontmatter from markdown content."""
    frontmatter = {}

    # Match --- delimited frontmatter
    match = re.match(r'^---\s*\n(.*?)\n---\s*\n', content, re.DOTALL)
    if not match:
        return frontmatter

    yaml_content = match.group(1)

    # Simple YAML parsing (no external deps)
    for line in yaml_content.split('\n'):
        line = line.strip()
        if not line or line.startswith('#'):
            continue

        if ':' in line:
            key, value = line.split(':', 1)
            key = key.strip()
            value = value.strip()

            # Parse arrays like [tag1, tag2] or - tag1
            if value.startswith('[') and value.endswith(']'):
                # Inline array: [tag1, tag2]
                items = value[1:-1].split(',')
                frontmatter[key] = [item.strip().strip('"\'') for item in items if item.strip()]
            elif value.startswith('"') and value.endswith('"'):
                frontmatter[key] = value[1:-1]
            elif value.startswith("'") and value.endswith("'"):
                frontmatter[key] = value[1:-1]
            elif value.lower() == 'true':
                frontmatter[key] = True
            elif value.lower() == 'false':
                frontmatter[key] = False
            elif value == '':
                # Key with no value, might be start of array
                frontmatter[key] = []
            else:
                frontmatter[key] = value
        elif line.startswith('- ') and frontmatter:
            # Array item continuation
            last_key = list(frontmatter.keys())[-1]
            if isinstance(frontmatter[last_key], list):
                frontmatter[last_key].append(line[2:].strip().strip('"\''))

    return frontmatter


def extract_title_from_heading(content: str) -> str | None:
    """Extract first # heading from markdown content."""
    # Skip frontmatter
    content_without_fm = re.sub(r'^---\s*\n.*?\n---\s*\n', '', content, flags=re.DOTALL)

    match = re.search(r'^#\s+(.+)$', content_without_fm, re.MULTILINE)
    return match.group(1).strip() if match else None


def parse_date_to_timestamp(value: str | int) -> int | None:
    """Parse date string or timestamp to Unix timestamp."""
    if isinstance(value, int):
        return value

    if not isinstance(value, str):
        return None

    # Try common date formats
    from datetime import datetime

    formats = [
        "%Y-%m-%d",           # 2024-01-15
        "%Y-%m-%dT%H:%M:%S",  # 2024-01-15T10:30:00
        "%Y/%m/%d",           # 2024/01/15
    ]

    for fmt in formats:
        try:
            dt = datetime.strptime(value, fmt)
            return int(dt.timestamp())
        except ValueError:
            continue

    return None


def get_git_modified_time(filepath: str) -> int | None:
    """Get git last modification timestamp for a file."""
    try:
        result = subprocess.run(
            ['git', 'log', '-1', '--format=%ct', '--', filepath],
            capture_output=True, text=True, check=True
        )
        return int(result.stdout.strip()) if result.stdout.strip() else None
    except (subprocess.CalledProcessError, ValueError):
        return None


def load_directory_meta(dir_path: Path) -> dict[str, Any]:
    """Load .meta.json from a directory if it exists."""
    meta_file = dir_path / ".meta.json"
    if meta_file.exists():
        try:
            with open(meta_file, 'r', encoding='utf-8') as f:
                return json.load(f)
        except (json.JSONDecodeError, IOError):
            pass
    return {}


def load_keys_file(filepath: Path) -> dict[str, Any] | None:
    """Load .keys sidecar file for encrypted files."""
    keys_file = filepath.with_suffix(filepath.suffix + '.keys')
    if keys_file.exists():
        try:
            with open(keys_file, 'r', encoding='utf-8') as f:
                return json.load(f)
        except (json.JSONDecodeError, IOError):
            pass
    return None


def process_file(filepath: Path, content_dir: Path) -> dict[str, Any] | None:
    """Process a single file and return its manifest entry."""
    # Skip .keys sidecar files
    if filepath.suffix.lower() == '.keys':
        return None

    if filepath.suffix.lower() not in EXTENSIONS:
        return None

    rel_path = str(filepath.relative_to(content_dir))

    # Skip hidden files/directories
    if any(part.startswith('.') for part in rel_path.split('/')):
        return None

    entry = {
        "path": rel_path,
        "title": Path(rel_path).stem,  # Default to filename without extension
        "size": filepath.stat().st_size,
        "modified": get_git_modified_time(str(filepath)),
        "tags": [],
    }

    # Check for encryption sidecar file (.keys)
    keys_data = load_keys_file(filepath)
    if keys_data:
        # Extract metadata from keys file
        if 'title' in keys_data:
            entry['title'] = keys_data['title']
        if 'tags' in keys_data and isinstance(keys_data['tags'], list):
            entry['tags'] = keys_data['tags']

        # Build encryption info
        encryption = {}
        if 'algorithm' in keys_data:
            encryption['algorithm'] = keys_data['algorithm']
        if 'wrapped_keys' in keys_data:
            encryption['wrapped_keys'] = keys_data['wrapped_keys']

        if encryption:
            entry['encryption'] = encryption

    # Parse markdown files for frontmatter and title (only if not encrypted)
    elif filepath.suffix.lower() == '.md':
        try:
            with open(filepath, 'r', encoding='utf-8') as f:
                content = f.read()

            # Parse frontmatter
            frontmatter = parse_frontmatter(content)

            # Title priority: frontmatter > # heading > filename
            if 'title' in frontmatter:
                entry['title'] = frontmatter['title']
            else:
                heading_title = extract_title_from_heading(content)
                if heading_title:
                    entry['title'] = heading_title

            # Tags from frontmatter
            if 'tags' in frontmatter and isinstance(frontmatter['tags'], list):
                entry['tags'] = frontmatter['tags']

            # Modified from frontmatter (overrides git timestamp)
            if 'modified' in frontmatter:
                entry['modified'] = parse_date_to_timestamp(frontmatter['modified'])

        except IOError:
            pass

    return entry


def process_directory_meta(dir_path: Path, content_dir: Path) -> dict[str, Any] | None:
    """Process a directory's .meta.json and return its manifest entry."""
    meta = load_directory_meta(dir_path)
    if not meta:
        return None

    rel_path = str(dir_path.relative_to(content_dir))
    if rel_path == '.':
        rel_path = ''

    entry = {
        "path": rel_path,
        "title": meta.get('title', dir_path.name if rel_path else 'Home'),
        "tags": meta.get('tags', []),
        "description": meta.get('description'),
        "icon": meta.get('icon'),
        "thumbnail": meta.get('thumbnail'),
    }

    return entry


def main():
    content_dir = Path(CONTENT_DIR)
    if not content_dir.exists():
        print(f"Content directory '{CONTENT_DIR}' not found")
        return

    files = []
    directories = []

    # Process all files
    for filepath in sorted(content_dir.rglob('*')):
        if filepath.is_file() and filepath.name != '.meta.json':
            entry = process_file(filepath, content_dir)
            if entry:
                files.append(entry)

    # Process directory metadata
    for dirpath in sorted(content_dir.rglob('*')):
        if dirpath.is_dir():
            entry = process_directory_meta(dirpath, content_dir)
            if entry:
                directories.append(entry)

    # Also check root directory
    root_entry = process_directory_meta(content_dir, content_dir)
    if root_entry:
        directories.append(root_entry)

    # Write manifest
    manifest = {
        "files": files,
        "directories": directories,
    }
    with open(OUTPUT_FILE, 'w', encoding='utf-8') as f:
        json.dump(manifest, f, indent=2, ensure_ascii=False)

    print(f"Generated {OUTPUT_FILE} with {len(files)} files and {len(directories)} directories")


if __name__ == "__main__":
    main()

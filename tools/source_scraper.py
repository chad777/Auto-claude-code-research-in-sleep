#!/usr/bin/env python3
"""Source Scraper — Fetch content from user-configured trusted websites.

Reads a sources configuration file and fetches/extracts content from
specified websites. Supports web search within trusted domains and
direct URL content extraction.

Usage:
    python source_scraper.py search "query"                    # Search all configured sources
    python source_scraper.py fetch "https://example.com/page"  # Fetch a specific URL
    python source_scraper.py list-sources                      # List configured sources
    python source_scraper.py add "name" "url" "category"       # Add a source to config
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import time
from typing import Any
from urllib.parse import urlparse

try:
    import httpx
    HAS_HTTPX = True
except ImportError:
    HAS_HTTPX = False
    httpx = None  # type: ignore

# Default config locations (in priority order)
DEFAULT_CONFIG_PATHS = [
    ".aris/sources.yml",
    ".aris/sources.json",
    "sources.yml",
    "sources.json",
]

# Built-in default sources (fallback if no config file exists)
DEFAULT_SOURCES = {
    "arxiv": {
        "url": "https://arxiv.org",
        "category": "research",
        "description": "Preprint repository for scientific papers",
        "search_url": "https://arxiv.org/search/?query={query}&searchtype=all"
    },
    "arxiv-sanity": {
        "url": "https://arxiv-sanity-lite.com",
        "category": "research",
        "description": "Curated arXiv paper recommendations",
        "search_url": "https://arxiv-sanity-lite.com/search?q={query}"
    },
    "huggingface-papers": {
        "url": "https://huggingface.co/papers",
        "category": "research",
        "description": "Daily trending ML papers on Hugging Face",
        "search_url": "https://huggingface.co/papers?search={query}"
    },
    "paperswithcode": {
        "url": "https://paperswithcode.com",
        "category": "research",
        "description": "Papers with code implementations and benchmarks",
        "search_url": "https://paperswithcode.com/search?q={query}"
    },
    "distill": {
        "url": "https://distill.pub",
        "category": "blog",
        "description": "Clear explanations of machine learning research",
        "search_url": "https://distill.pub/search/?q={query}"
    },
    "openreview": {
        "url": "https://openreview.net",
        "category": "research",
        "description": "Open peer review platform for conferences",
        "search_url": "https://openreview.net/search?term={query}"
    },
    "github": {
        "url": "https://github.com",
        "category": "code",
        "description": "Open source code and repositories",
        "search_url": "https://github.com/search?q={query}&type=repositories"
    },
    "medium": {
        "url": "https://medium.com",
        "category": "blog",
        "description": "Technical blog posts and tutorials",
        "search_url": "https://medium.com/search?q={query}"
    },
    "towards-ds": {
        "url": "https://towardsdatascience.com",
        "category": "blog",
        "description": "Data science articles and tutorials",
        "search_url": "https://towardsdatascience.com/search?q={query}"
    },
    "stackoverflow": {
        "url": "https://stackoverflow.com",
        "category": "community",
        "description": "Q&A for programmers and technical questions",
        "search_url": "https://stackoverflow.com/search?q={query}"
    },
}

# ---- Config Management ----

def find_config() -> tuple[str, dict] | None:
    """Find and load the sources config file."""
    for rel_path in DEFAULT_CONFIG_PATHS:
        candidates = [rel_path]
        if os.path.exists(rel_path):
            pass  # relative path works
        # Also check repo root
        if "ARIS_REPO" in os.environ:
            repo_path = os.path.join(os.environ["ARIS_REPO"], rel_path)
            candidates.append(repo_path)
        for path in candidates:
            if os.path.exists(path):
                with open(path, "r", encoding="utf-8") as f:
                    if path.endswith(".json"):
                        data = json.load(f)
                        return path, data.get("sources", data)
                    else:
                        # Simple YAML-like parser
                        return path, _parse_yaml_simple(f.read())
    return None

def _parse_yaml_simple(text: str) -> dict:
    """Simple key-value parser for YAML-like source configs."""
    sources = {}
    current_key = None
    current_source = {}
    in_source = False

    for line in text.split("\n"):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("- name:"):
            if current_key and current_source:
                sources[current_key] = current_source
            current_key = line.split(":", 1)[1].strip()
            current_source = {"name": current_key}
            in_source = True
        elif in_source and ":" in line:
            k, v = line.split(":", 1)
            k = k.strip().lstrip("- ")
            v = v.strip()
            if k in ("name", "name"):
                pass  # already set
            else:
                current_source[k] = v

    if current_key and current_source:
        sources[current_key] = current_source

    return sources or DEFAULT_SOURCES

def write_config(sources: dict, path: str = ".aris/sources.json") -> None:
    """Write sources to JSON config file."""
    os.makedirs(os.path.dirname(path) if os.path.dirname(path) else ".", exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump({"sources": sources}, f, indent=2, ensure_ascii=False)
    print(f"Sources saved to {path}")

# ---- Web Fetching ----

def fetch_url(url: str, timeout: int = 30) -> dict[str, Any]:
    """Fetch content from a URL."""
    if not HAS_HTTPX:
        return {"error": "httpx not installed. Run: pip install httpx", "url": url}

    headers = {
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    }

    try:
        with httpx.Client(timeout=timeout, follow_redirects=True) as client:
            response = client.get(url, headers=headers)
            response.raise_for_status()
            content_type = response.headers.get("content-type", "")

            if "text/html" in content_type:
                text = response.text
                # Basic text extraction
                text = re.sub(r"<script[^>]*>.*?</script>", "", text, flags=re.DOTALL)
                text = re.sub(r"<style[^>]*>.*?</style>", "", text, flags=re.DOTALL)
                text = re.sub(r"<[^>]+>", " ", text)
                text = re.sub(r"\s+", " ", text).strip()
                text = text[:50000]  # Limit to 50K chars
            else:
                text = response.text[:50000]

            return {
                "url": url,
                "status": response.status_code,
                "content": text,
                "length": len(text),
                "content_type": content_type,
            }

    except httpx.TimeoutException:
        return {"error": f"Timeout fetching {url}", "url": url}
    except httpx.HTTPStatusError as e:
        return {"error": f"HTTP {e.response.status_code}: {url}", "url": url}
    except Exception as e:
        return {"error": str(e), "url": url}

def search_source(source: dict, query: str) -> dict[str, Any]:
    """Search a specific source for the query."""
    search_url = source.get("search_url", "")
    name = source.get("name", source.get("url", "unknown"))

    if not search_url:
        return {"source": name, "error": "No search_url configured"}

    # Replace {query} with URL-encoded query
    import urllib.parse
    encoded_query = urllib.parse.quote(query)
    url = search_url.replace("{query}", encoded_query)

    result = fetch_url(url)
    result["source"] = name
    result["source_url"] = source.get("url", "")
    result["source_category"] = source.get("category", "general")
    return result

# ---- CLI ----

def cmd_search(args):
    """Search configured sources."""
    config = find_config()
    if config:
        path, sources = config
        print(f"Using sources from: {path}")
    else:
        sources = DEFAULT_SOURCES
        print(f"Using {len(sources)} built-in default sources")

    results = []
    for name, source in sources.items():
        source["name"] = source.get("name", name)
        result = search_source(source, args.query)
        results.append(result)
        print(f"  [{result.get('source', name)}] {'✅' if 'content' in result else '❌ ' + result.get('error', 'unknown')} ({result.get('length', 0)} chars)")

    output = {"query": args.query, "sources_checked": len(results), "results": results}
    if args.output:
        with open(args.output, "w", encoding="utf-8") as f:
            json.dump(output, f, indent=2, ensure_ascii=False)
        print(f"\nResults saved to: {args.output}")
    else:
        print(json.dumps(output, indent=2, ensure_ascii=False)[:2000] + "...")

def cmd_fetch(args):
    """Fetch a specific URL."""
    result = fetch_url(args.url)
    if "error" in result:
        print(f"Error: {result['error']}")
    else:
        print(f"URL: {result['url']}")
        print(f"Status: {result['status']}")
        print(f"Content: {result['length']} chars")
        print(f"\n--- Content Preview ---")
        print(result["content"][:2000])

def cmd_list(args):
    """List configured sources."""
    config = find_config()
    if config:
        path, sources = config
        print(f"Sources from: {path}")
    else:
        sources = DEFAULT_SOURCES
        print(f"Built-in default sources ({len(sources)}):")

    print(f"\n{'Name':<20} {'Category':<15} {'URL':<40}")
    print("-" * 75)
    for name, source in sorted(sources.items()):
        cat = source.get("category", "general")
        url = source.get("url", "")
        print(f"{name:<20} {cat:<15} {url:<40}")

def cmd_add(args):
    """Add a source to the config."""
    config = find_config()
    if config:
        path, sources = config
    else:
        path = ".aris/sources.json"
        sources = dict(DEFAULT_SOURCES)

    source_id = args.name.lower().replace(" ", "-")
    sources[source_id] = {
        "url": args.url,
        "category": args.category,
        "description": args.description or f"User-added source: {args.url}",
        "search_url": args.search_url or f"{args.url.rstrip('/')}/search?q={{query}}",
    }
    write_config(sources, path)
    print(f"Added source: {source_id} ({args.url})")

def build_parser():
    parser = argparse.ArgumentParser(description="Source Scraper — Fetch from trusted websites")
    sub = parser.add_subparsers(dest="command", required=True)

    s = sub.add_parser("search", help="Search all configured sources")
    s.add_argument("query")
    s.add_argument("--output", "-o", help="Save results to file")

    f = sub.add_parser("fetch", help="Fetch a specific URL")
    f.add_argument("url")

    l = sub.add_parser("list-sources", help="List configured sources")

    a = sub.add_parser("add", help="Add a new source to config")
    a.add_argument("name")
    a.add_argument("url")
    a.add_argument("--category", default="general", help="Category: research/blog/news/code/community/general")
    a.add_argument("--description", help="Short description")
    a.add_argument("--search-url", help="Search URL template with {query}")

    return parser

def main():
    parser = build_parser()
    args = parser.parse_args()

    if args.command == "search":
        cmd_search(args)
    elif args.command == "fetch":
        cmd_fetch(args)
    elif args.command == "list-sources":
        cmd_list(args)
    elif args.command == "add":
        cmd_add(args)

if __name__ == "__main__":
    main()

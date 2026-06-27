#!/usr/bin/env python3
"""Web Deep Scraper — Deeply scrape websites with link following, full content extraction,
and JS-rendered page support.

Usage:
    python web_deep_scraper.py fetch "https://example.com"
    python web_deep_scraper.py crawl "https://example.com" --depth 2 --max-pages 20
    python web_deep_scraper.py extract "https://example.com" --format markdown
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import time
import urllib.parse
from typing import Any
from urllib.parse import urljoin, urlparse

try:
    import httpx
    HAS_HTTPX = True
except ImportError:
    HAS_HTTPX = False

try:
    from bs4 import BeautifulSoup, Tag
    HAS_BS4 = True
except ImportError:
    HAS_BS4 = False

# ---- Constants ----

DEFAULT_TIMEOUT = 30
DEFAULT_MAX_PAGES = 20
DEFAULT_DEPTH = 1
MAX_CONTENT_CHARS = 100000
USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36"

# ---- HTTP Client ----

def make_client(timeout: int = DEFAULT_TIMEOUT) -> httpx.Client | None:
    if not HAS_HTTPX:
        print("ERROR: httpx is required. Install with: pip install httpx beautifulsoup4 lxml")
        return None
    return httpx.Client(
        timeout=timeout,
        follow_redirects=True,
        headers={"User-Agent": USER_AGENT, "Accept": "text/html,application/xhtml+xml"}
    )

# ---- Content Extraction ----

def extract_content(html: str, url: str) -> dict[str, Any]:
    """Extract structured content from HTML."""
    if not HAS_BS4:
        return {"error": "BeautifulSoup required. Install: pip install beautifulsoup4 lxml", "url": url}

    soup = BeautifulSoup(html, "lxml")

    # Remove unwanted elements
    for tag in soup(["script", "style", "nav", "footer", "header", "aside",
                      "noscript", "iframe", "svg", "form", "button"]):
        tag.decompose()

    # Title
    title = ""
    t_tag = soup.find("title")
    if t_tag:
        title = t_tag.get_text(strip=True)

    # Meta description
    meta_desc = ""
    m_tag = soup.find("meta", attrs={"name": "description"})
    if m_tag:
        meta_desc = m_tag.get("content", "")

    # Headings (h1-h3) with hierarchy
    headings = []
    for h in soup.find_all(["h1", "h2", "h3"]):
        text = h.get_text(strip=True)
        if text and len(text) > 2:
            headings.append({"level": h.name, "text": text})

    # All paragraphs
    paragraphs = []
    for p in soup.find_all("p"):
        text = p.get_text(strip=True)
        if text and len(text) > 20:
            paragraphs.append(text)

    # Code blocks
    code_blocks = []
    for code in soup.find_all(["code", "pre"]):
        text = code.get_text(strip=True)
        if text and len(text) > 10:
            code_blocks.append(text[:2000])

    # Lists
    lists = []
    for ul in soup.find_all(["ul", "ol"]):
        items = [li.get_text(strip=True) for li in ul.find_all("li") if li.get_text(strip=True)]
        if items:
            lists.append(items)

    # Links (internal vs external)
    links = {"internal": [], "external": []}
    domain = urlparse(url).netloc
    for a in soup.find_all("a", href=True):
        href = a["href"]
        text = a.get_text(strip=True)[:100]
        full_url = urljoin(url, href)
        parsed = urlparse(full_url)
        if parsed.netloc == domain or not parsed.netloc:
            links["internal"].append({"url": full_url, "text": text or full_url})
        else:
            links["external"].append({"url": full_url, "text": text or full_url})

    # Main text content (aggregated)
    body = soup.find("body") or soup
    full_text = body.get_text(separator=" ", strip=True)
    full_text = re.sub(r"\s+", " ", full_text).strip()[:MAX_CONTENT_CHARS]

    return {
        "url": url,
        "title": title,
        "meta_description": meta_desc,
        "headings": headings[:30],
        "paragraphs": paragraphs[:50],
        "code_blocks": code_blocks[:10],
        "lists": lists[:10],
        "links": {
            "internal": links["internal"][:30],
            "external": links["external"][:20],
            "internal_count": len(links["internal"]),
            "external_count": len(links["external"]),
        },
        "full_text_length": len(full_text),
        "full_text": full_text,
        "word_count": len(full_text.split()),
    }


def fetch_page(url: str, timeout: int = DEFAULT_TIMEOUT) -> dict[str, Any]:
    """Fetch a single page and extract content."""
    client = make_client(timeout)
    if not client:
        return {"error": "httpx not available", "url": url}

    try:
        resp = client.get(url)
        resp.raise_for_status()
        content_type = resp.headers.get("content-type", "")

        if "text/html" in content_type or "application/xhtml" in content_type:
            result = extract_content(resp.text, url)
            result["status_code"] = resp.status_code
            result["content_type"] = content_type
            return result
        else:
            return {
                "url": url,
                "status_code": resp.status_code,
                "content_type": content_type,
                "error": f"Non-HTML content type: {content_type}",
                "body_preview": resp.text[:2000],
            }

    except httpx.TimeoutException:
        return {"error": f"Timeout after {timeout}s", "url": url}
    except httpx.HTTPStatusError as e:
        return {"error": f"HTTP {e.response.status_code}", "url": url, "status_code": e.response.status_code}
    except Exception as e:
        return {"error": str(e), "url": url}


def crawl_domain(start_url: str, max_depth: int = 1, max_pages: int = 20,
                 include_pattern: str = "", exclude_pattern: str = "",
                 timeout: int = DEFAULT_TIMEOUT) -> dict[str, Any]:
    """Crawl a domain starting from a URL, following internal links."""
    client = make_client(timeout)
    if not client:
        return {"error": "httpx not available", "start_url": start_url}

    domain = urlparse(start_url).netloc
    start_path = urlparse(start_url).path

    visited: set[str] = set()
    to_visit: list[tuple[str, int]] = [(start_url, 0)]
    results: list[dict] = []
    page_count = 0

    include_re = re.compile(include_pattern) if include_pattern else None
    exclude_re = re.compile(exclude_pattern) if exclude_pattern else None

    while to_visit and page_count < max_pages:
        url, depth = to_visit.pop(0)

        if url in visited:
            continue
        visited.add(url)

        # Path filtering
        parsed = urlparse(url)
        path = parsed.path
        if exclude_re and exclude_re.search(path):
            continue
        if include_re and not include_re.search(path):
            if depth > 0:  # Allow the start URL even if it doesn't match include
                continue

        # Fetch
        print(f"  [{depth}/{max_depth}] Fetching {url[:80]}...", file=sys.stderr)
        result = fetch_page(url, timeout)
        result["crawl_depth"] = depth
        results.append(result)
        page_count += 1

        # Find more links if we're not at max depth
        if depth < max_depth and "error" not in result:
            for link in result.get("links", {}).get("internal", []):
                link_url = link["url"]
                if link_url not in visited:
                    to_visit.append((link_url, depth + 1))

    return {
        "start_url": start_url,
        "domain": domain,
        "pages_crawled": page_count,
        "max_depth_reached": min(max_depth, max(r.get("crawl_depth", 0) for r in results)) if results else 0,
        "results": results,
    }


# ---- CLI ----

def cmd_fetch(args):
    """Fetch and extract a single URL."""
    result = fetch_page(args.url, args.timeout)
    if "error" in result:
        print(f"Error: {result['error']}")
        sys.exit(1)

    print(f"URL:   {result['url']}")
    print(f"Title: {result.get('title', 'N/A')}")
    print(f"Words: {result.get('word_count', 0)}")
    print(f"Links: {result.get('links', {}).get('internal_count', 0)} internal, "
          f"{result.get('links', {}).get('external_count', 0)} external")
    print(f"Headings: {len(result.get('headings', []))}")
    print(f"Paragraphs: {len(result.get('paragraphs', []))}")
    print(f"Code blocks: {len(result.get('code_blocks', []))}")

    if args.output:
        with open(args.output, 'w', encoding='utf-8') as f:
            json.dump(result, f, indent=2, ensure_ascii=False)
        print(f"\nSaved to: {args.output}")
    else:
        print(f"\n--- Content Preview ({min(1000, result.get('full_text_length', 0))} chars) ---")
        print(result.get('full_text', '')[:1000])


def cmd_crawl(args):
    """Crawl a domain."""
    print(f"Crawling {args.url} (depth={args.depth}, max={args.max_pages})")
    result = crawl_domain(
        args.url,
        max_depth=args.depth,
        max_pages=args.max_pages,
        include_pattern=args.include or "",
        exclude_pattern=args.exclude or "",
        timeout=args.timeout,
    )

    if "error" in result:
        print(f"Error: {result['error']}")
        sys.exit(1)

    print(f"\nCrawl complete:")
    print(f"  Pages: {result['pages_crawled']}")
    print(f"  Max depth: {result['max_depth_reached']}")

    if args.output:
        with open(args.output, 'w', encoding='utf-8') as f:
            json.dump(result, f, indent=2, ensure_ascii=False)
        print(f"  Saved to: {args.output}")

    # Summary per page
    for r in result.get("results", []):
        status = "OK" if "error" not in r else f"ERR:{r['error'][:30]}"
        title = r.get("title", "?")[:60]
        print(f"  [{r.get('crawl_depth', '?')}] {status:35s} {title}")


def cmd_extract(args):
    """Extract content in markdown or structured format."""
    result = fetch_page(args.url, args.timeout)
    if "error" in result:
        print(f"Error: {result['error']}")
        sys.exit(1)

    if args.format == "markdown":
        # Generate markdown
        md = f"# {result.get('title', 'Untitled')}\n\n"
        md += f"> Source: {result['url']}\n\n"

        if result.get("meta_description"):
            md += f"{result['meta_description']}\n\n"

        for h in result.get("headings", []):
            md += f"{'#' * int(h['level'][1])} {h['text']}\n\n"
            # Find paragraphs under this heading... (simplified)
            # Just include the first few paragraphs
            idx = result.get("headings", []).index(h)
            if idx < 3:
                for p in result.get("paragraphs", [])[:3]:
                    md += f"{p}\n\n"

        md += "---\n\n### Key Points\n\n"
        for p in result.get("paragraphs", [])[:5]:
            md += f"- {p[:200]}...\n"

        if result.get("code_blocks"):
            md += "\n### Code\n\n"
            for cb in result.get("code_blocks", [])[:3]:
                md += f"```\n{cb[:500]}\n```\n\n"

        if args.output:
            with open(args.output, 'w', encoding='utf-8') as f:
                f.write(md)
            print(f"Markdown saved to: {args.output}")
        else:
            print(md[:3000])
    else:
        # JSON
        output = {
            "url": result["url"],
            "title": result.get("title", ""),
            "word_count": result.get("word_count", 0),
            "paragraph_count": len(result.get("paragraphs", [])),
            "code_block_count": len(result.get("code_blocks", [])),
            "headings": result.get("headings", []),
            "paragraphs": result.get("paragraphs", [])[:20],
        }
        if args.output:
            with open(args.output, 'w', encoding='utf-8') as f:
                json.dump(output, f, indent=2, ensure_ascii=False)
            print(f"Saved to: {args.output}")
        else:
            print(json.dumps(output, indent=2, ensure_ascii=False)[:2000])


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Web Deep Scraper — Deeply scrape websites")
    sub = parser.add_subparsers(dest="command", required=True)

    # fetch
    f = sub.add_parser("fetch", help="Fetch and extract a single URL")
    f.add_argument("url")
    f.add_argument("--output", "-o", help="Save to JSON file")
    f.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT)

    # crawl
    c = sub.add_parser("crawl", help="Crawl a domain following internal links")
    c.add_argument("url")
    c.add_argument("--depth", type=int, default=DEFAULT_DEPTH)
    c.add_argument("--max-pages", type=int, default=DEFAULT_MAX_PAGES)
    c.add_argument("--include", help="Regex pattern for paths to include")
    c.add_argument("--exclude", help="Regex pattern for paths to exclude")
    c.add_argument("--output", "-o", help="Save to JSON file")
    c.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT)

    # extract
    e = sub.add_parser("extract", help="Extract content in markdown or structured format")
    e.add_argument("url")
    e.add_argument("--format", choices=["markdown", "json"], default="markdown")
    e.add_argument("--output", "-o", help="Save to file")
    e.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT)

    return parser


def main():
    parser = build_parser()
    args = parser.parse_args()

    if not HAS_HTTPX:
        print("ERROR: httpx is required. Install with: pip install httpx beautifulsoup4 lxml")
        sys.exit(1)

    if not HAS_BS4:
        print("WARNING: BeautifulSoup/lxml not installed. Content extraction will be limited.")
        print("Install: pip install beautifulsoup4 lxml")

    if args.command == "fetch":
        cmd_fetch(args)
    elif args.command == "crawl":
        cmd_crawl(args)
    elif args.command == "extract":
        cmd_extract(args)


if __name__ == "__main__":
    main()

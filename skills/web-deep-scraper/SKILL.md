---
name: web-deep-scraper
description: "Deeply scrape websites with full content extraction, link following, and markdown output. Fetch single pages, crawl entire domains, or extract structured content. Use when you need to research content from a specific website beyond basic search results."
argument-hint: "[url] [-- action: fetch|crawl|extract] [-- depth: 1]"
allowed-tools: Bash(*), Read, Write
---

# Web Deep Scraper — Full Website Content Extraction

Target: $ARGUMENTS

## Overview

Extract rich content from websites — fetch single pages, crawl domains, or extract structured data in markdown format. Complements `/source-scraper` (which searches curated sources) by letting you dive deep into any specific website.

### Role & Positioning

| Skill | What It Does | Best For |
|-------|-------------|----------|
| `/source-scraper` | Search curated list of trusted sites | Broad research across known sources |
| **`/web-deep-scraper`** | Deep scrape any single URL or domain | Diving into one specific website |
| `/exa-search` | Web search with content extraction | Finding pages you don't know about |
| `/youtube-research` | YouTube video content | Video tutorials and talks |

## Constants

- **SCRAPER_FETCHER** = `web_deep_scraper.py` — Canonical name, resolved per integration-contract
- **OUTPUT_DIR** = `research-output/web/` — Where scraped content is saved
- **DEFAULT_DEPTH** = 1 — How deep to follow links
- **MAX_PAGES** = 20 — Maximum pages to crawl

> Overrides (append to arguments):
> - `/web-deep-scraper "https://example.com"` — Fetch single page (default)
> - `/web-deep-scraper "https://example.com" — action: extract, format: markdown` — Markdown output
> - `/web-deep-scraper "https://example.com" — action: crawl, depth: 2, max-pages: 10` — Crawl domain
> - `/web-deep-scraper "https://example.com/docs" — action: crawl, include: "/docs/"` — Only /docs/ pages

## Actions

### Action: `fetch` (default)

Fetch a single URL and extract structured content:

```
/web-deep-scraper "https://example.com/article"
```

Returns:
- Title, meta description, headings hierarchy
- All paragraphs, code blocks, links
- Full text content
- Word count and link counts

Output saved to `{OUTPUT_DIR}/pages/{domain}_{timestamp}.json`

### Action: `extract`

Same as fetch but output as formatted markdown:

```
/web-deep-scraper "https://example.com" — action: extract, format: markdown
```

Useful for saving research notes in readable format.

### Action: `crawl`

Follow internal links to a specified depth:

```
/web-deep-scraper "https://example.com/docs" — action: crawl, depth: 2, max-pages: 20
```

Features:
- Respects `--depth` (how many link hops)
- Respects `--max-pages` (max pages to fetch)
- `--include` regex — only follow matching paths
- `--exclude` regex — skip matching paths
- Deduplicates visited URLs automatically

Output saved to `{OUTPUT_DIR}/crawls/{domain}_{timestamp}.json`

## Workflow

### Step 1: Parse Arguments

Parse $ARGUMENTS:
- **URL**: The target URL (required)
- **`— action: fetch`** — Single page (default)
- **`— action: extract`** — Markdown output
- **`— action: crawl`** — Multi-page domain crawl
- **`— depth: N`** — Link-following depth (crawl only)
- **`— max-pages: N`** — Max pages (crawl only)
- **`— include: PATTERN`** — Include path regex (crawl only)
- **`— exclude: PATTERN`** — Exclude path regex (crawl only)

### Step 2: Execute

Resolve `$SCRAPER_FETCHER` via the canonical chain:

```bash
cd "$(git rev-parse --show-toplevel 2>/dev/null || echo .)"
if [ -d ".aris/tools" ]; then T="$HOME/.aris/tools"
elif [ -d "tools" ]; then T="tools"
elif [ -n "$ARIS_REPO" ] && [ -d "$ARIS_REPO/tools" ]; then T="$ARIS_REPO/tools"
else echo "ERROR: Cannot find web_deep_scraper.py"; exit 1; fi

python3 "$T/web_deep_scraper.py" [action] [url] [flags]
```

### Step 3: Process Results

- Save to OUTPUT_DIR with timestamp
- Present summary to user:
  ```
  Web Scrape: [title]
  URL:        [url]
  Content:    [N] words, [N] paragraphs, [N] code blocks
  Links:      [N] internal, [N] external
  Saved to:   research-output/web/pages/...
  ```

## Dependencies

- `httpx` — HTTP client (already installed)
- `beautifulsoup4` — HTML parsing (install: pip install beautifulsoup4 lxml)
- `lxml` — Fast HTML parser (install: pip install lxml)

## Example Usage

```bash
# Fetch a single article
/web-deep-scraper "https://lilianweng.github.io/posts/2023-01-27-the-transformer-family-v2/"

# Crawl documentation site
/web-deep-scraper "https://docs.anthropic.com/en/docs" — action: crawl, depth: 1

# Extract as markdown notes
/web-deep-scraper "https://distill.pub/2021/multimodal-neurons/" — action: extract, format: markdown
```

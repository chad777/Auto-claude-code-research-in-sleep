---
name: source-scraper
description: "Search and fetch content from a configurable list of trusted websites. Manage your personal research source library — add sites like Distill.pub, PapersWithCode, StackOverflow, or any custom URL, then search across them or fetch specific pages."
argument-hint: query-or-url
allowed-tools: Bash(*), Read, Write
---

# Source Scraper — Curated Website Research

Query: $ARGUMENTS

## Overview

Search across your **personal library of trusted websites** or fetch content from specific URLs. Unlike generic web search, this skill targets only sources you've curated, giving you higher signal-to-noise for specialized research.

### When to Use

- You have a list of trusted blogs, documentation sites, or repositories
- You want to search across specific websites (not the whole web)
- You want to add a new source to your research pipeline
- You need to extract content from a specific URL

### Built-In Default Sources

| Source | Category | URL |
|--------|----------|-----|
| arXiv | research | arxiv.org |
| HuggingFace Papers | research | huggingface.co/papers |
| PapersWithCode | research | paperswithcode.com |
| Distill | blog | distill.pub |
| OpenReview | research | openreview.net |
| GitHub | code | github.com |
| Medium | blog | medium.com |
| Towards Data Science | blog | towardsdatascience.com |
| StackOverflow | community | stackoverflow.com |

You can add your own with: `/source-scraper add "name" "url"`

## Constants

- **SOURCE_FETCHER** = `source_scraper.py` — Canonical name, resolved per integration-contract
- **OUTPUT_DIR** = `research-output/sources/`
- **MAX_CHARS_PER_SOURCE** = 30000 — Max chars to extract per source

> Overrides (append to arguments):
> - `/source-scraper "query"` — Search all configured sources
> - `/source-scraper fetch "https://example.com"` — Fetch a specific URL
> - `/source-scraper list-sources` — List all configured sources
> - `/source-scraper add "blog-name" "https://blog.example.com"` — Add a new source
> - `/source-scraper "query" — output: results.json` — Save to file

## Workflow

### Step 1: Parse Arguments

Parse $ARGUMENTS to determine action:

- **Plain query**: Search all configured sources
- **`fetch URL`**: Fetch a specific URL
- **`list-sources`**: List configured sources
- **`add name URL`**: Add a new source

### Step 2: Execute

Resolve `$SOURCE_FETCHER` via the canonical chain:
```bash
cd "$(git rev-parse --show-toplevel 2>/dev/null || echo .)"
if [ -d ".aris/tools" ]; then T="$HOME/.aris/tools"
elif [ -d "tools" ]; then T="tools"
elif [ -n "$ARIS_REPO" ] && [ -d "$ARIS_REPO/tools" ]; then T="$ARIS_REPO/tools"
else echo "ERROR: Cannot find source_scraper.py"; exit 1; fi

python3 "$T/source_scraper.py" [action] [args]
```

### Step 3: Process Results

For search results:
- Save full JSON to `{OUTPUT_DIR}/{timestamp}_search.json`
- Extract and summarize key content from each source
- Present a digest: "Found info on {query} from {N} sources"

For URL fetch:
- Save extracted content to `{OUTPUT_DIR}/pages/{domain}_{timestamp}.txt`

### Step 4: Present

Show a summary table:
```
Source Search: "[query]"
─────────────────────────
arXiv           ✅ 2 papers found
Distill         ✅ 1 article found
GitHub          ✅ 3 repos found
StackOverflow   ❌ No results
```

## Output Structure

```
research-output/sources/
├── {timestamp}_search.json     # Raw search results
└── pages/
    └── {domain}_{timestamp}.txt  # Individual page content
```

## Managing Your Source Library

### Add a Source
```
/source-scraper add "my-blog" "https://myblog.com" — category: blog
```

### Default Config Location
The source config is stored at `.aris/sources.json` in your project, or you can create it manually.

Example `.aris/sources.json`:
```json
{
  "sources": {
    "my-custom-site": {
      "url": "https://docs.example.com",
      "category": "docs",
      "description": "Custom documentation site",
      "search_url": "https://docs.example.com/search?q={query}"
    }
  }
}
```

## Dependency

- `httpx` — For HTTP requests (install: `pip install httpx`)

---
name: general-research
description: "End-to-end multi-source research pipeline: from any topic through YouTube, web, curated sources, and optional papers -- then cross-validated by multi-model debate and compiled into a structured report. Use when you want thorough research on any non-paper topic, or when you want to supplement paper research with video/web/social sources."
argument-hint: "[research-topic] [-- sources: youtube,web,papers] [-- effort: lite|balanced|max]"
allowed-tools: Bash(*), Read, Write, Grep, Glob, WebSearch, WebFetch, mcp__openrouter-debate__debate, Skill
---

# General Research Pipeline: Multi-Source Research -> Cross-Model Debate -> Report

Research topic: **$ARGUMENTS**

## Overview

This skill chains YouTube research, web source scraping, optional paper search, and multi-model debate into a single automated pipeline:

```
$ARGUMENTS
    |
    +-- Phase 1: DISPATCH (auto-classify query)
    |   +-- Selects sources based on query type
    |
    +-- Phase 2: COLLECT (parallel source gathering)
    |   +-- /youtube-research "topic"       <- YouTube videos + transcripts
    |   +-- /source-scraper "topic"         <- Curated websites (arXiv, GitHub, etc.)
    |   +-- /web-deep-scraper "url"        <- Deep scrape specific pages
    |   +-- [/research-lit "topic"]         <- Academic papers (if requested)
    |
    +-- Phase 3: SYNTHESIZE (compile findings)
    |   +-- Create FINDINGS.md from all sources
    |
    +-- Phase 4: DEBATE (cross-model validation)
    |   +-- /openrouter-debate -> consensus report
    |
    +-- Phase 5: REPORT (final output)
        +-- /research-report -> RESEARCH_REPORT.md
```

## Constants

- **OUTPUT_DIR** = `research-output/` -- Root output directory
- **AUTO_PROCEED** = `true` -- If true, automatically proceed through phases. Set `-- auto-proceed: false` to pause at each phase.
- **RUN_DEBATE** = `true` -- If true, run cross-model debate after collecting findings. Set `-- debate: false` to skip.
- **DEFAULT_SOURCES** -- Auto-selected based on query classification (see Phase 1)
- **DEBATE_PANEL** -- Default panel: Critic (Gemini 3.1 Flash Lite) + Synthesizer (Mistral Medium) + DeepDiver (DeepSeek Chat) -> Arbiter (Gemini 3.5 Flash)

> Overrides (append to arguments):
> - `-- effort: lite` -- Quick: 3 YouTube, 3 sources, skip debate
> - `-- effort: balanced` -- Standard: 5 YouTube, 6 sources, full debate (default)
> - `-- effort: max` -- Deep: 10 YouTube, all sources, full debate + deep arbiter
> - `-- sources: youtube,web` -- Only YouTube + web (skip papers)
> - `-- sources: all` -- YouTube + web + papers + social
> - `-- debate: false` -- Skip multi-model debate, just compile findings
> - `-- output: my-report.md` -- Custom output filename

## Phase 1: Query Classification & Dispatch

Parse $ARGUMENTS to classify the query and select sources:

### Classification Logic

Examine the query text for indicators:

| Indicator | Query Type | Sources Activated |
|-----------|-----------|-------------------|
| paper, arxiv, published, conference, journal, SOTA, benchmark, method, algorithm, survey | ACADEMIC | papers + web |
| how to, tutorial, guide, setup, install, configure, example, beginner | EDUCATIONAL | YouTube + web |
| news, latest, update, announced, released, this week, trends | NEWS | web + YouTube |
| opinion, discussion, controversy, debate, community | SOCIAL | web |
| tool, library, framework, software, open source, github, alternative to | TOOLS | YouTube + web + GitHub |
| anything else | GENERAL | YouTube + web + papers |

### Effort Levels

| Effort | YouTube Videos | Web Sources | Papers | Debate |
|--------|---------------|-------------|--------|--------|
| `lite` | 3 | 4 | 5 | Skip |
| `balanced` (default) | 5 | 6 | 10 | Full panel + arbiter |
| `max` | 10 | All | 20 | Full panel + arbiter (deep) |

## Phase 2: Parallel Collection

### Step 2a: YouTube Research

If YouTube sources are active:

```
/youtube-research "$TOPIC" -- transcripts: true, max: $MAX_VIDEOS
```

Wait for completion. Output saved to `{OUTPUT_DIR}/youtube/`.

### Step 2b: Deep Web Scraping (Optional)

If specific URLs are provided via `-- urls:`:

```
/web-deep-scraper "$URL" -- action: extract, format: markdown
```

Or crawl a documentation site:
```
/web-deep-scraper "$URL" -- action: crawl, depth: 1, max-pages: 10
```

### Step 2c: Web Source Scraping

```
/source-scraper "$TOPIC" -- output: {OUTPUT_DIR}/sources/results.json
```

Or directly:
```bash
python3 "$ARIS_REPO/tools/source_scraper.py" search "$TOPIC"
```

### Step 2d: Paper Search (Optional)

If papers are active:

```
/research-lit "$TOPIC" -- sources: arxiv, semantic-scholar -- max: $MAX_PAPERS
```

## Phase 3: Synthesize Findings

Collect all outputs and compile FINDINGS.md from:
- `{OUTPUT_DIR}/youtube/youtube_findings.json`
- `{OUTPUT_DIR}/sources/results.json`
- `{OUTPUT_DIR}/papers/` (if present)

## Phase 4: Multi-Model Debate

If RUN_DEBATE is true:

```
/openrouter-debate "$TOPIC" -- findings: {OUTPUT_DIR}/FINDINGS.md
```

Wait for debate to complete. Output saved to `{OUTPUT_DIR}/debate/DEBATE_REPORT.md`.

## Phase 5: Generate Report

```
/research-report "$TOPIC"
```

Produces `{OUTPUT_DIR}/RESEARCH_REPORT.md`.

## Output Structure

```
research-output/
+-- FINDINGS.md                  # Compiled raw findings
+-- youtube/
|   +-- youtube_findings.json    # YouTube results
+-- sources/
|   +-- results.json             # Web source results
+-- papers/                      # (if papers were searched)
+-- debate/
|   +-- DEBATE_REPORT.md         # Multi-model debate output
+-- RESEARCH_REPORT.md           # Final synthesized report
```

## Example Usage

```bash
# Quick research on a technical topic
/general-research "LoRA fine-tuning for LLMs" -- effort: lite

# Deep multi-source research
/general-research "Mamba vs Transformer 2026" -- effort: max

# Focus on specific sources
/general-research "attention mechanisms" -- sources: youtube, web

# Skip debate, just collect
/general-research "RLHF techniques" -- debate: false

# Academic focus
/general-research "survey on state space models" -- sources: papers, web
```

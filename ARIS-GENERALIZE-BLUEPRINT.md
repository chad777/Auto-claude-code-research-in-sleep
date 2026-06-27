# ARIS Generalization Blueprint

> **Phase 1 of 4** — Design document for extending ARIS from paper-only research to a general multi-source research platform.
> Created for: wanshuiyin/Auto-claude-code-research-in-sleep (ARIS)
> Author: Hermes Agent
> Versions: v1.0

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [System Architecture](#2-system-architecture)
3. [New Source Skills](#3-new-source-skills)
4. [Generalized Pipeline](#4-generalized-pipeline)
5. [Multi-Model Debate Layer via OpenRouter](#5-multi-model-debate-layer-via-openrouter)
6. [Context-Aware Tool Dispatch](#6-context-aware-tool-dispatch)
7. [Implementation Roadmap](#7-implementation-roadmap)
8. [File Structure & Artifacts](#8-file-structure--artifacts)
9. [Contracts & Governance](#9-contracts--governance)
10. [Windows Setup Notes](#10-windows-setup-notes)

---

## 1. Executive Summary

### Goal

Transform ARIS from a **paper-only research system** (79 skills for ML paper lifecycle) into a **general-purpose multi-source research engine** that can investigate any topic — academic or not — by pulling from YouTube, websites, social media, and papers, then cross-validating findings through multiple LLMs via OpenRouter.

### Design Principles

1. **Composability** — Every new source is a standalone skill following ARIS's existing patterns. No framework changes.
2. **Adversarial validation** — Multiple model families from OpenRouter debate findings. Never one model judging its own work.
3. **Context-aware dispatch** — The system auto-selects which sources and models to use based on the query type.
4. **Backward compatibility** — Existing paper workflows (W1-W6) untouched. `/research-pipeline` still works exactly as before.

### Key Insight: Most Infrastructure Already Exists

| Need | ARIS Status |
|------|------------|
| Skill system | ✅ 79 skills, composable markdown |
| Cross-model review | ✅ Codex MCP, Gemini-review, Claude-review, Oracle MCP |
| Multi-provider support | ✅ 10 documented routes (Alt A-I) |
| OpenRouter integration | ✅ `llm-chat` MCP server + docs |
| Runtime isolation | ✅ `tools/` pattern, `integration-contract.md` |
| Output tracking | ✅ `MANIFEST.md`, `output-manifest.md` |
| File structure | ✅ `/skills/`, `/tools/`, `/mcp-servers/` |

---

## 2. System Architecture

### Current Architecture (Papers Only)

```
/research-lit ──→ /arxiv
               ├── /semantic-scholar
               ├── /deepxiv
               ├── /alphaxiv
               ├── /exa-search
               ├── /gemini-search
               └── /openalex
```

### Proposed Architecture (General Research)

```
/general-research "topic" ──→ DISPATCH (auto-detects context)
                               │
                               ├── ACADEMIC SOURCES
                               │   ├── /arxiv
                               │   ├── /semantic-scholar
                               │   ├── /deepxiv / alphaxiv
                               │   ├── /openalex
                               │   └── /exa-search (paper filter)
                               │
                               ├── MEDIA SOURCES  (★ NEW)
                               │   ├── /youtube-research
                               │   ├── /podcast-research
                               │   └── /video-transcript
                               │
                               ├── WEB SOURCES    (★ NEW)
                               │   ├── /web-deep-research
                               │   ├── /exa-search (general filter)
                               │   └── /rss-watcher
                               │
                               ├── SOCIAL SOURCES (★ NEW)
                               │   ├── /reddit-research
                               │   ├── /twitter-research
                               │   └── /hackernews-research
                               │
                               └── SYNTHESIS LAYER
                                   ├── /openrouter-debate (★ NEW)
                                   │   → Model A finds facts
                                   │   → Model B challenges
                                   │   → Model C arbitrates
                                   │   → Consensus output
                                   └── /research-report (★ NEW)
                                       → Unified findings document
```

---

## 3. New Source Skills

### 3.1 `/youtube-research`

**Purpose:** Fetch YouTube video transcripts, summarize video content, extract key claims, and feed into research pipeline.

**Interface:** Standard ARIS skill with parameters:
```
/youtube-research "topic" — max: 10, type: educational
```

**Backend:** Python tool `tools/youtube_fetch.py`
- Uses `yt-dlp` (installed separately) for transcript extraction
- OR YouTube Data API v3 for search + metadata
- OR `youtube-transcript-api` Python library (lightweight, no auth needed for transcripts)

**Data flow:**
```
Query → YouTube search (api) → Get video IDs → Fetch transcripts
→ Chunk + summarize per video → Extract claims/themes
→ Output: youtube-research/FINDINGS.md
```

**Dependencies:** `pip install yt-dlp youtube-transcript-api` (or `brew install yt-dlp`)

### 3.2 `/web-deep-research`

**Purpose:** Deep scrape websites beyond Exa's search — fetch full page content, follow links within a domain, extract structured data.

**Interface:**
```
/web-deep-research "https://example.com" — depth: 2, max pages: 20
```

**Backend:** Python tool `tools/web_deep_fetch.py`
- Uses `httpx` + `beautifulsoup4` for scraping
- Or `crawl4ai` for JS-rendered pages
- Respects `robots.txt`
- Extracts: title, headings, body text, links, metadata

**Dependencies:** `pip install httpx beautifulsoup4 lxml`

### 3.3 `/reddit-research`

**Purpose:** Search Reddit for discussions, opinions, and community knowledge on a topic.

**Interface:**
```
/reddit-research "topic" — subreddit: MachineLearning, sort: top, time: year
```

**Backend:** Python tool `tools/reddit_fetch.py`
- Uses Pushshift.io API (free, no auth) or Reddit's official API
- Searches submissions + top comments
- Summarizes community consensus and controversial points

**Dependencies:** `pip install praw` (for official Reddit API) or simple `httpx` for Pushshift

### 3.4 `/hackernews-research`

**Purpose:** Search Hacker News for technical discussions, Show HN projects, and community knowledge.

**Interface:**
```
/hackernews-research "topic" — min points: 50, time: month
```

**Backend:** Python tool `tools/hackernews_fetch.py`
- Uses Algolia HN Search API (free, no auth)
- Returns stories with points, comments, and top comment excerpts

**Dependencies:** `httpx` (stdlib-compatible)

### 3.5 `/podcast-research`

**Purpose:** Search podcast transcripts for research content.

**Interface:**
```
/podcast-research "topic"
```

**Backend:** Python tool `tools/podcast_fetch.py`
- Searches Listen Notes API or other podcast transcript indexes
- Extracts relevant segments

---

## 4. Generalized Pipeline

### 4.1 New Skill: `/general-research`

**Location:** `skills/general-research/SKILL.md`

This is the primary entry point for non-paper research. It:
1. Auto-detects the nature of the query
2. Selects appropriate sources
3. Fans out to all selected sources in parallel
4. Collects findings
5. Passes to OpenRouter for multi-model debate
6. Produces a unified research report

**Phases:**

| Phase | Skill | Purpose |
|-------|-------|---------|
| 1. Dispatch | inline | Classify query → select sources |
| 2. Collect | /research-lit + new skills | Gather raw data from all sources |
| 3. Analyze | /openrouter-debate | Cross-model verification |
| 4. Synthesize | /research-report | Write unified findings |

**Query Classification Logic:**

```
IF query contains: paper, arxiv, published, conference, journal, SOTA, benchmark
  → ACADEMIC MODE (papers + some web)

IF query contains: tutorial, how-to, guide, explanation, course
  → EDUCATIONAL MODE (YouTube + web + papers)

IF query contains: opinion, discussion, controversy, community, feels like
  → SOCIAL MODE (Reddit + HN + web)

IF query contains: news, latest, update, trend, announcement
  → NEWS MODE (web + social + YouTube)

DEFAULT → GENERAL MODE (all sources)
```

### 4.2 New Skill: `/research-report`

**Location:** `skills/research-report/SKILL.md`

Generates a structured research report from mixed-source findings:

```markdown
# Research Report: [Topic]

## Executive Summary

## Sources Used
- YouTube: [N] videos
- Web: [N] pages
- Reddit/HN: [N] threads
- Academic: [N] papers

## Key Findings
- [Finding 1] (verified by Model A, B, C)
- [Finding 2] (disputed by Model B — see debate log)
- ...

## Cross-Model Verdict
- Consensus points
- Disputed points
- Confidence scores per finding

## Source Raw Data
- Transcript excerpts
- Web page excerpts
- Paper abstracts

## Debate Log
- Link to openrouter-debate output
```

---

## 5. Multi-Model Debate Layer via OpenRouter

### 5.1 New Skill: `/openrouter-debate`

**Location:** `skills/openrouter-debate/SKILL.md`

**Purpose:** Take raw research findings and run them through a panel of different LLMs via OpenRouter to cross-validate, challenge, and produce consensus.

**Architecture:**

```
┌─────────────────────────────────────────────────────┐
│                  /openrouter-debate                    │
│                                                       │
│  RAW FINDINGS                                          │
│       │                                                │
│       ▼                                                │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐             │
│  │ Model A  │  │ Model B  │  │ Model C  │             │
│  │ (Claude  │  │ (GPT-4o) │  │ (DeepSeek│             │
│  │  Sonnet) │  │          │  │  V3)     │             │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘             │
│       │             │             │                    │
│       ▼             ▼             ▼                    │
│  Individual answers (all see same question)            │
│       │             │             │                    │
│       └─────────────┼─────────────┘                    │
│                     ▼                                  │
│  ┌──────────────────────────────────────┐              │
│  │        Arbiter Model (Model D)        │              │
│  │  Compares A, B, C → consensus         │              │
│  └──────────────────────────────────────┘              │
│                     ▼                                  │
│              DEBATE_REPORT.md                           │
└──────────────────────────────────────────────────────┘
```

**Model Panel Config:**

Each instance of /openrouter-debate selects 3-4 models from different families:

| Slot | Default | Purpose |
|------|---------|---------|
| A | `anthropic/claude-sonnet-4` | Reasoning + caution |
| B | `openai/gpt-4o-2024-11-20` | Breadth + speed |
| C | `deepseek/deepseek-chat` | Technical depth |
| Arbiter (D) | `google/gemini-2.0-flash-001` | Neutral arbiter |

All configurable via `— models: a:claude-sonnet-4, b:gpt-4o, c:deepseek-chat, arbiter:gemini-flash`.

**Debate Format:**

Each model receives:
```
RESEARCH QUESTION: [topic]

RAW FINDINGS:
[concatenated findings from all sources]

TASK: Analyze the findings above. For each claim:
1. Is it well-supported by the evidence?
2. Are there counter-arguments or missing context?
3. How confident are you (1-10)?
4. What additional sources would help?

Output your response as structured JSON.
```

Then the arbiter receives:
```
Below are answers from 3 different AI models analyzing the same research findings.

MODEL A (Claude) response: [...]
MODEL B (GPT-4o) response: [...]
MODEL C (DeepSeek) response: [...]

TASK: Compare these three analyses. Identify:
1. Where all three agree → mark as CONSENSUS
2. Where they disagree → mark as DISPUTED, explain differences
3. Produce a final synthesized verdict
```

### 5.2 MCP Server for OpenRouter

**Location:** `mcp-servers/openrouter-debate/server.py`

New lightweight MCP server specifically for multi-model debate. Built on top of the existing `llm-chat` pattern but with parallel model calls.

**Key difference from `llm-chat`:** This server:
- Accepts a list of models to call
- Calls them all in parallel (async httpx)
- Returns structured JSON with per-model answers
- Provides a `debate` tool that takes findings + question + model list + arbiter

**Dependencies:** `pip install httpx`

### 5.3 Existing Infrastructure We Leverage

| Component | What It Already Does |
|-----------|---------------------|
| `mcp-servers/llm-chat/server.py` | Single-model OpenAI-compatible calling |
| `docs/OPENROUTER_GUIDE.md` | Full setup guide for OpenRouter API key + `llm-chat` |
| `skills/auto-review-loop-llm/SKILL.md` | Existing OpenRouter-backed review loop (Alt C.1) |
| `docs/LLM_API_MIX_MATCH_GUIDE.md` | Mix-and-match model configurations |

---

## 6. Context-Aware Tool Dispatch

### 6.1 New Skill: `/research-dispatch`

**Location:** `skills/research-dispatch/SKILL.md`

**Purpose:** Intelligent dispatcher that classifies a research query and selects only the relevant sources.

**Dispatch Logic:**

```
Step 1: Parse query and check for explicit source override
  IF /research-dispatch "topic" — sources: youtube, web
    → Use only listed sources

Step 2: Auto-classify query
  Analyze $ARGUMENTS for keywords and structure:

  Academic indicators: "paper", "arxiv", "SOTA", "benchmark", "method", "algorithm",
                       "published in", "conference", "journal", "survey"
    → ACADEMIC (arxiv, semantic-scholar, deepxiv, openalex, exa)

  Technical how-to indicators: "how to", "tutorial", "guide", "setup",
                               "install", "configure", "example"
    → EDUCATIONAL (youtube, web, exa)

  Social/Discussion indicators: "opinion", "discussion", "community",
                                "controversy", "trending", "what do people think"
    → SOCIAL (reddit, HN, exa, web)

  News/Current indicators: "news", "latest", "update", "announced", "released",
                           "yesterday", "this week", "2026"
    → NEWS (web, exa, social)

  Project/Tool indicators: "tool", "library", "framework", "software",
                           "open source", "github"
    → TOOLS (web, youtube, reddit, HN)

  General/Default:
    → ALL sources, with paper-only disabled
```

**Dispatch as a Router Skill:**

`/research-dispatch` doesn't do research itself. It:
1. Classifies the query
2. Prints the classification result
3. Recommends which `/general-research` configuration to use
4. OR auto-chains into `/general-research` with the right parameters

### 6.2 Source Selection Matrix

| Query Type | Papers | YouTube | Web | Reddit | HN | Twitter |
|---|---|---|---|---|---|---|
| Academic | ✅ | ❌ | ⚠️ | ❌ | ❌ | ❌ |
| Educational | ❌ | ✅ | ✅ | ⚠️ | ❌ | ❌ |
| Social | ❌ | ❌ | ⚠️ | ✅ | ✅ | ✅ |
| News | ❌ | ✅ | ✅ | ✅ | ✅ | ⚠️ |
| Tools | ❌ | ✅ | ✅ | ✅ | ✅ | ❌ |
| General | ⚠️ | ✅ | ✅ | ✅ | ✅ | ❌ |

Legend: ✅ Primary | ⚠️ Supplemental | ❌ Skip

---

## 7. Implementation Roadmap

### Phase 1: Setup & Tooling (this guide's next step)
- [ ] Install Claude Code + Codex CLI on Windows
- [ ] Clone ARIS repo
- [ ] Install ARIS skills (symlinks)
- [ ] Set up OpenRouter API key
- [ ] Verify basic `/research-lit` works

### Phase 2: OpenRouter Debate Layer (Option 4)
- [ ] Build `mcp-servers/openrouter-debate/server.py`
- [ ] Register OpenRouter debate MCP in Claude Code
- [ ] Create `/openrouter-debate` skill
- [ ] Test: same query to Claude + GPT + DeepSeek → compare
- [ ] Create `/research-report` skill

### Phase 3: YouTube Research (Option 1)
- [ ] Build `tools/youtube_fetch.py`
- [ ] Create `/youtube-research` skill
- [ ] Test: fetch + summarize video on a research topic

### Phase 4: Web & Social Skills
- [ ] Build `tools/web_deep_fetch.py`
- [ ] Build `tools/reddit_fetch.py`
- [ ] Build `tools/hackernews_fetch.py`
- [ ] Create `/web-deep-research`, `/reddit-research`, `/hackernews-research` skills

### Phase 5: Dispatch & General Pipeline
- [ ] Create `/research-dispatch` skill
- [ ] Create `/general-research` pipeline skill
- [ ] Test: end-to-end general research query
- [ ] Verify backward compatibility with existing paper workflows

---

## 8. File Structure & Artifacts

### New Files to Create

```
aris_repo/
├── skills/
│   ├── general-research/
│   │   └── SKILL.md
│   ├── research-dispatch/
│   │   └── SKILL.md
│   ├── research-report/
│   │   └── SKILL.md
│   ├── openrouter-debate/
│   │   └── SKILL.md
│   ├── youtube-research/
│   │   └── SKILL.md
│   ├── web-deep-research/
│   │   └── SKILL.md
│   ├── reddit-research/
│   │   └── SKILL.md
│   └── hackernews-research/
│       └── SKILL.md
├── tools/
│   ├── youtube_fetch.py
│   ├── web_deep_fetch.py
│   ├── reddit_fetch.py
│   └── hackernews_fetch.py
├── mcp-servers/
│   └── openrouter-debate/
│       ├── server.py
│       └── requirements.txt
└── docs/
    └── GENERAL_RESEARCH_GUIDE.md
```

### Output Artifacts (within a research project)

```
project/
├── research-output/
│   ├── FINDINGS.md              # Raw collected findings from all sources
│   ├── DEBATE_REPORT.md         # Multi-model debate output
│   └── RESEARCH_REPORT.md       # Final synthesized report
├── youtube-research/
│   ├── VIDEOS.md                # Video list + summaries
│   └── transcripts/
├── web-research/
│   ├── PAGES.md                 # Page summaries
│   └── content/
└── social-research/
    ├── REDFIND.md               # Reddit findings
    └── HN.md                    # Hacker News findings
```

---

## 9. Contracts & Governance

### Following ARIS Standards

Every new skill MUST follow these ARIS conventions:

1. **Integration Contract** (`shared-references/integration-contract.md`)
   - Canonical helpers in `tools/`
   - Resolver block for finding tools at runtime

2. **External Cadence** (`shared-references/external-cadence.md`)
   - Never wrap verdict-bearing skills in external loops
   - `/openrouter-debate` is verdict-bearing — use once, not on a timer

3. **Reviewer Independence** (`shared-references/reviewer-independence.md`)
   - Debate models must be different families from each other
   - Executor ≠ reviewer must hold

4. **Effort / Assurance Axes**
   - `— effort: lite | balanced | max | beast`
   - `— assurance: draft | polished | conference-ready | submission`
   - General research uses same axes, adapted: `draft` = quick scan, `polished` = deep with debate

5. **Output Manifest**
   - All major output files get MANIFEST.md entries

6. **Skill Governance**
   - Tools auto-generate `.provenance.json` for audit trail

---

## 10. Windows Setup Notes

### Known Issues

The ARIS repo is designed primarily for macOS (bash scripts, `install_aris.sh`, symlinks). Windows needs workarounds:

| Issue | Workaround |
|-------|------------|
| `install_aris.sh` (bash) | Use git-bash or WSL; or use `install_aris.ps1` (PowerShell version exists at `tools/install_aris.ps1`) |
| Symlinks require admin | Run PowerShell as Admin, or use `mklink` |
| Path separator (`/` vs `\`) | Python handles this fine; bash scripts may need `MSYS_NO_PATHCONV=1` |
| `.claude/` directory | Lives under `%USERPROFILE%\.claude\` on Windows |
| LaTeX | Install MiKTeX for Windows |
| Claude Code | `npm install -g @anthropic-ai/claude-code` |
| Codex CLI | `npm install -g @openai/codex` |

### Recommended Windows Setup Path

```
1. Install Node.js (for Claude Code + Codex CLI)
2. Open git-bash as Admin for symlink support
3. Clone ARIS repo
4. Run install_aris.ps1 (PowerShell version)
5. Set environment variables via System Properties
6. Test: claude --version && codex --version
```

---

## Appendix: Existing ARIS Skills Catalog (Relevant)

| Skill | Role | Already Does |
|-------|------|-------------|
| `/research-lit` | Core literature search | Papers from 7+ sources |
| `/exa-search` | Web search with content extraction | Broad web, blogs, docs, news |
| `/gemini-search` | AI-powered literature discovery | Beyond keyword matching |
| `/research-review` | Cross-model critical review | Codex MCP / manual backends |
| `/idea-creator` | Brainstorming from literature | Generates ranked ideas |
| `/novelty-check` | Verify idea novelty | Against existing literature |
| `/meta-optimize` | Auto-improve ARIS skills | Skill self-improvement |
| `/auto-review-loop` | Iterative review + fix | Until positive or max rounds |
| `/auto-review-loop-llm` | OpenRouter-backed review | Alt C.1 reviewer path |
| `/arxiv` | Direct arxiv search | Preprint search + PDF download |
| `/comm-lit-review` | Domain-specific lit review | Communications papers |



---

## Project Completion Status

### Phase 1: Blueprint ✅
- [x] Comprehensive design document written
- [x] Architecture, skills, data flow defined
- [x] Windows setup notes included

### Phase 2: Windows ARIS Setup ✅
- [x] Claude Code verified (v2.1.186)
- [x] ARIS skills installed (103 skills)
- [x] `llm-chat` MCP server configured → OpenRouter free models
- [x] `ARIS_REPO` environment variable set
- [x] `httpx` dependency installed

### Phase 3: OpenRouter Multi-Model Debate Layer ✅
- [x] `mcp-servers/openrouter-debate/server.py` — Async MCP server for parallel multi-model debate
- [x] `skills/openrouter-debate/SKILL.md` — Multi-model debate skill
- [x] `skills/research-report/SKILL.md` — Research report generator
- [x] Registered in `.mcp.json` for Claude Code
- [x] Panel: MiniMax (critic) + Gemini (synthesizer) + DeepSeek (deep dive) → MiniMax (arbiter)
- [x] All free-tier models on OpenRouter

### Phase 4: YouTube Research Skill ✅
- [x] `tools/youtube_fetch.py` — Python tool for YouTube search + transcript extraction
- [x] `skills/youtube-research/SKILL.md` — YouTube research skill
- [x] Search verified (returns metadata successfully)
- [x] Transcript verified (88K chars extracted successfully)
- [x] Installed to `~/.claude/skills/youtube-research/`

### New Files Created

```
aris_repo/
├── ARIS-GENERALIZE-BLUEPRINT.md           # This document
├── mcp-servers/openrouter-debate/
│   ├── server.py                          # Multi-model debate MCP server
│   └── requirements.txt                   # httpx dependency
├── skills/
│   ├── openrouter-debate/SKILL.md         # Multi-model debate skill
│   ├── research-report/SKILL.md           # Research report generator
│   └── youtube-research/SKILL.md          # YouTube research skill
└── tools/
    └── youtube_fetch.py                   # YouTube search + transcript tool
```

### Files Modified

```
~/.claude/
├── .mcp.json                              # Added llm-chat + openrouter-debate MCPs
└── settings.json                          # Added ARIS_REPO, OPENROUTER_API_KEY, LLM_* vars
```

### Phase 5: General Research Pipeline
- [x] `/general-research` pipeline skill created
- [x] Chaining: YouTube -> Sources -> Papers -> Debate -> Report
- [x] Query classification logic (academic/educational/news/social/tools/general)
- [x] Effort levels: lite / balanced / max
- [x] Installed to `~/.claude/skills/general-research/`

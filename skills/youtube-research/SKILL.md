---
name: youtube-research
description: "Search YouTube videos, fetch transcripts, and extract research insights from video content. Use when you want to research a topic through video content — tutorials, lectures, talks, or technical deep-dives."
argument-hint: topic-or-search-query
allowed-tools: Bash(*), Read, Write
---

# YouTube Research

Research topic: $ARGUMENTS

## Overview

Searches YouTube for relevant videos on a topic, fetches transcripts, and extracts research insights. Complements paper-based research by capturing:
- Tutorials and how-to guides
- Conference talks and lecture series
- Technical deep-dives and walkthroughs
- Panel discussions and interviews

## Role & Positioning

| Skill | Source | Best for |
|-------|--------|----------|
| `/arxiv` | arXiv API | Preprints and papers |
| `/youtube-research` | YouTube | Video tutorials, talks, technical content |
| `/research-lit` | Mixed papers | Full literature survey |
| `/exa-search` | Exa API | Broad web search |

## Constants

- **YOUTUBE_FETCHER** — Canonical name `youtube_fetch.py`, resolved per integration-contract.md §2
- **MAX_RESULTS** = 5 — Default number of videos to search
- **OUTPUT_DIR** = `research-output/youtube/` — Where results are saved
- **MAX_TRANSCRIPT_CHARS** = 15000 — Max transcript chars to include in summary (truncate if longer)
- **FETCH_TRANSCRIPTS** = `true` — When true, fetch transcripts for all results. Set `— transcripts: false` to skip.

> Overrides (append to arguments):
> - `/youtube-research "topic" — max: 10` — top 10 videos
> - `/youtube-research "topic" — transcripts: false` — just list videos, skip transcripts
> - `/youtube-research "topic" — type: lecture` — filter by content type (appended to query)

## Workflow

### Step 1: Parse Arguments

Parse $ARGUMENTS for:
- **Query**: The search topic
- **`— max: N`**: Override MAX_RESULTS
- **`— transcripts: false`**: Skip transcript fetching
- **`— dir: PATH`**: Override OUTPUT_DIR

### Step 2: Search YouTube

Resolve $YOUTUBE_FETCHER via the canonical chain:
```bash
cd "$(git rev-parse --show-toplevel 2>/dev/null || echo .)"
aris_tools_dir=""
if [ -d ".aris/tools" ]; then aris_tools_dir=".aris/tools"
elif [ -d "tools" ]; then aris_tools_dir="tools"
elif [ -n "$ARIS_REPO" ] && [ -d "$ARIS_REPO/tools" ]; then aris_tools_dir="$ARIS_REPO/tools"
else echo "ERROR: Cannot find youtube_fetch.py"; exit 1; fi

python3 "$aris_tools_dir/youtube_fetch.py" search "$QUERY" --max $MAX_RESULTS
```

Save results to `{OUTPUT_DIR}/VIDEOS.md` as a structured markdown table.

### Step 3: Fetch Transcripts (Optional)

If FETCH_TRANSCRIPTS = true:
```bash
python3 "$aris_tools_dir/youtube_fetch.py" search-transcript "$QUERY" --max $MAX_RESULTS
```

For each video, save the transcript to `{OUTPUT_DIR}/transcripts/{video_id}.txt`
and create a summary of key points.

### Step 4: Extract Insights

From the transcripts, extract:
- Key claims made
- Technical details and methods discussed
- Code examples or implementations shown
- References to papers or other sources
- Contradictions or debates between different videos

Save as `{OUTPUT_DIR}/INSIGHTS.md`.

### Step 5: Present Results

Show the user:
```
YouTube Research: "[topic]"
─────────────────────────────────
Found {N} videos:
  - {title} ({channel}) — {duration}s
  - ...

Transcripts: {fetched/skipped}
Insights: {N} key points extracted
Sources saved to: research-output/youtube/
```

## Output Structure

```
research-output/youtube/
├── VIDEOS.md              # Video list with metadata (markdown table)
├── transcripts/
│   ├── video1_id.txt     # Raw transcript
│   └── video2_id.txt
└── INSIGHTS.md            # Extracted insights and key points
```

## Example Usage

```
/youtube-research "Transformer attention mechanisms explained"
    → Search + fetch transcripts + extract insights

/youtube-research "LoRA fine-tuning tutorial" — max: 3, transcripts: false
    → Just list top 3 videos, skip transcripts

/youtube-research "RLHF vs DPO comparison" — max: 10
    → Deep dive: 10 videos with transcripts
```

## Dependencies

- `yt-dlp` — Video metadata and transcript extraction
  Install: `pip install yt-dlp` or `winget install yt-dlp`
- Optional: `youtube-transcript-api` for fallback transcript fetching
  Install: `pip install youtube-transcript-api`

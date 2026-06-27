---
name: website-clone-research
description: "Research any website by cloning it. Uses the AI Website Cloner Template to reverse-engineer a site into Next.js, then analyzes the extracted design, content, and structure for research purposes. Combining website deep research with cloned analysis for competitor research, design pattern analysis, or content preservation."
argument-hint: "<url> [-- output: ./cloned-site] [-- analyze: true]"
allowed-tools: Bash(*), Read, Write, Grep, Glob, WebSearch, WebFetch
---

# Website Clone Research: Reverse-Engineer + Analyze Any Site

Target URL: $ARGUMENTS

## Overview

This skill clones a website using the [AI Website Cloner Template](https://github.com/JCodesMore/ai-website-cloner-template) and then analyzes the cloned output for research insights. It's useful for:

- **Competitor research** — understand design patterns, content strategy, and UX decisions
- **Design pattern analysis** — extract and study component structures
- **Content preservation** — save a local copy of a site for offline analysis
- **Technical research** — study how a site is built (tech stack, component architecture)

### How It Differs From Other Skills

| Skill | What It Does | Best For |
|-------|-------------|----------|
| `/web-deep-scraper` | Scrape/extract page content | Text content, articles |
| `/source-scraper` | Search across curated sources | Finding information |
| **`/website-clone-research`** | Full site clone + analysis | Design, structure, UX analysis |

## Prerequisites

1. **Node.js 24+** — Check with `node --version`
2. **Claude Code** (or another supported AI agent) — Installed and authenticated
3. **Browser automation** — Chrome MCP, Playwright, or Puppeteer (the cloner needs screenshots)

## Constants

- **CLONER_REPO** = `https://github.com/JCodesMore/ai-website-cloner-template`
- **DEFAULT_OUTPUT_DIR** = `cloned-sites/` — Where cloned sites are stored
- **RUN_ANALYSIS** = `true` — Run research analysis after cloning. Set `-- analyze: false` to skip.

> Overrides:
> - `-- output: ./my-clone` — Custom output directory
> - `-- analyze: false` — Clone only, no analysis
> - `-- deps-only: true` — Only install dependencies, don't clone yet (for prep)
> - `-- tech: nextjs` — Override the target tech stack

## Workflow

### Phase 1: Setup Clone Project

Set up the cloner project in your output directory:

```bash
# Step 1: Create your project from the template
#   Go to https://github.com/JCodesMore/ai-website-cloner-template
#   Click "Use this template" -> "Create a new repository"
#   OR clone it directly for research:

git clone $CLONER_REPO $OUTPUT_DIR
cd $OUTPUT_DIR

# Step 2: Install dependencies
npm install
```

### Phase 2: Clone the Target Website

Launch your AI agent inside the project directory:

```bash
cd $OUTPUT_DIR

# Launch Claude Code with browser support
claude --chrome
```

Inside Claude Code, run:

```
/clone-website <target-url>
```

This will:
1. Take screenshots of the target site
2. Extract design tokens (colors, fonts, spacing)
3. Download all assets
4. Write component specifications
5. Dispatch parallel builders to reconstruct each section in Next.js

**For multiple URLs:**
```
/clone-website <url1> <url2> <url3>
```

### Phase 3: Research Analysis

After cloning completes (or if analysis mode is on), analyze the cloned output:

```bash
cd $OUTPUT_DIR

# Review the research artifacts
ls docs/research/
ls docs/design-references/
```

Extract research data from:

1. **Design Tokens** (`docs/research/`) — fonts, colors, spacing system
2. **Component Specs** (`docs/research/components/`) — detailed component breakdown
3. **Screenshots** (`docs/design-references/`) — visual references
4. **Source Code** (`src/`) — the reconstructed implementation

### Phase 4: Compile Research Report

Compile findings into a research report:

```markdown
# Website Research Report: [target-url]

## Overview
- **URL:** [target]
- **Clone Date:** [date]
- **Tech Stack:** [detected: Next.js + Tailwind + shadcn]
- **Pages Cloned:** [count]

## Design Analysis
### Color System
- Primary: [extracted colors]
- Typography: [fonts used]
- Spacing: [detected spacing scale]

### Component Architecture
[Key components identified with their structure]

### Content Analysis
[Main content sections and their purpose]

### UX Patterns
[Interaction patterns, layouts, responsive behavior]

## Technical Findings
- Framework detection
- Asset inventory
- Performance indicators

## Local Artifacts
- Cloned project: [path]
- Screenshots: [path]
- Component specs: [path]
```

### Phase 5: Save to Research Output

Copy the analysis to your ARIS research output:

```bash
cp docs/research/RESEARCH_REPORT.md $ARIS_REPO/research-output/cloned-sites/
```

## Output Structure

After cloning a site, the project directory contains:

```
cloned-site/
+-- src/                        # Reconstructed source code
|   +-- app/                    # Pages
|   +-- components/             # Rebuilt components
|       +-- ui/                 # shadcn/ui primitives
+-- docs/
|   +-- research/               # Research artifacts
|   |   +-- design-tokens.md    # Extracted design tokens
|   |   +-- components/         # Component specifications
|   +-- design-references/      # Screenshots and references
|       +-- comparison.png      # Before/after comparison
+-- public/
    +-- images/                 # Downloaded assets
```

## Example Usage

```bash
# Research a competitor's landing page
/website-clone-research "https://example.com" -- output: ./cloned-sites/example

# Quick setup (deps only, you'll run the clone later)
/website-clone-research "https://example.com" -- deps-only: true

# Clone multiple URLs for comparison
/website-clone-research "https://site1.com https://site2.com"

# Clone only, skip analysis
/website-clone-research "https://example.com" -- analyze: false
```

## Dependencies

- Node.js 24+
- Claude Code (`npm install -g @anthropic-ai/claude-code`)
- Chrome or Chromium (for screenshots)
- An AI agent that supports the cloner template (Claude Code recommended)

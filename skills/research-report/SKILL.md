---
name: research-report
description: "Synthesize raw research findings into a structured multi-source research report. Combines findings from papers, web, YouTube, and social sources, optionally runs through OpenRouter multi-model debate, and produces a unified report."
argument-hint: topic-or-scope
allowed-tools: Bash(*), Read, Write, Grep, Glob, mcp__openrouter-debate__debate
---

# Research Report Generator

Topic: $ARGUMENTS

## Overview

Generates a structured research report from mixed-source findings. Can optionally pipe findings through the OpenRouter multi-model debate layer for cross-validation.

### Input Sources

Checks for findings in this order:
1. `research-output/FINDINGS.md` — consolidated findings from all sources
2. `research-output/DEBATE_REPORT.md` — if debate already run, incorporate it
3. Individual source directories:
   - `research-output/youtube/`
   - `research-output/web/`
   - `research-output/social/`
   - `idea-stage/` — ARIS idea output

## Constants

- **OUTPUT_DIR** = `research-output/`
- **REPORT_FILE** = `research-output/RESEARCH_REPORT.md`
- **RUN_DEBATE** = `true` — When `true`, automatically run OpenRouter debate on findings before writing report. Set `— run-debate: false` to skip.
- **MAX_SOURCE_EXCERPTS** = 5 — Max excerpts per source type in the report.

## Workflow

### Phase 1: Gather Findings

Scan for all available source data. Create a `FINDINGS.md` if one doesn't exist by concatenating:
- Any `.md` files in `research-output/` subdirectories
- Parse for key claims, quotes, and data points

### Phase 2: Multi-Model Debate (Optional)

If `RUN_DEBATE = true` and no `DEBATE_REPORT.md` exists:
1. Call `mcp__openrouter-debate__debate` with question + findings
2. Save output as `DEBATE_REPORT.md`
3. Extract consensus/disputed points

### Phase 3: Write Report

Write a structured report covering:

```markdown
# Research Report: [Topic]

## Executive Summary
[2-3 paragraph overview of key findings]

## Sources Used
| Source Type | Count | Details |
|-------------|-------|---------|
| Research Papers | N | arxiv, S2, OpenAlex |
| YouTube Videos | N | [titles] |
| Web Pages | N | [domains] |
| Social Media | N | Reddit, HN |

## Key Findings
### Finding 1: [Claim]
- **Evidence**: [What supports this]
- **Confidence**: [High/Medium/Low — based on debate consensus]
- **Counter-arguments**: [If any]
- **Sources**: [Links to original sources]

### Finding 2: ...
...

## Cross-Model Verification
### Consensus Points
[What all models agreed on]

### Disputed Points
[Where models disagreed — flag for human review]

## Open Questions
[What still needs investigation]

## Recommendations
[Actionable next steps based on findings]

## Appendix
- Full source list with links
- Debate transcript
- Methodology notes
```

### Phase 4: Save and Present

Save to `REPORT_FILE` and present a summary to the user.

## Example Usage

```bash
/research-report "Transformer attention mechanisms"
    # Scans research-output/ for findings, runs debate, produces report

/research-report "Topic" — run-debate: false
    # Skip debate, just compile findings into a report
```

## Output

- `research-output/RESEARCH_REPORT.md` — The full report
- `research-output/DEBATE_REPORT.md` — (if debate was run)
- `research-output/FINDINGS.md` — (if was created from source scan)

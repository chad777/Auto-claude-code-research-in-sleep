---
name: openrouter-debate
description: "Run a multi-model debate on research findings. Calls multiple LLMs via OpenRouter in parallel, then an arbiter model produces a consensus verdict. Use when you want cross-model validation of research findings, fact-checking, or multi-perspective analysis."
argument-hint: topic-or-scope
allowed-tools: Bash(*), Read, Write, mcp__openrouter-debate__debate
---

# OpenRouter Multi-Model Debate

Research topic: $ARGUMENTS

## Overview

This skill takes research findings and runs them through a panel of different LLMs via OpenRouter. Each model analyzes the findings independently, then an arbiter model produces a consensus verdict.

### Architecture

```
FINDINGS → Panelist A (MiniMax) →          → Panelist B (Gemini)   → → Arbiter (MiniMax) → DEBATE_REPORT.md
         → Panelist C (DeepSeek) → /
```

### When to Use

- Cross-validate findings from multiple sources
- Fact-check claims against counter-arguments
- Get multi-perspective analysis on controversial topics
- Before publishing a research report, run it through debate for rigor

### When NOT to Use

- For single-model analysis (use /research-review or /auto-review-loop-llm instead)
- In a scheduled loop — debate is verdict-bearing and should run once

## Constants

- **OUTPUT_DIR** = `research-output/` — Where debate reports are saved
- **DEBATE_SERVER** = `mcp__openrouter-debate__debate` — MCP tool name
- **DEFAULT_PANEL**:
  - Critic: `google/gemini-3.1-flash-lite` — finds flaws and gaps
  - Synthesizer: `mistralai/mistral-medium-3-5` — finds patterns and connections
  - DeepDiver: `deepseek/deepseek-chat` — rigorous evidence analysis
  - Arbiter: `google/gemini-3.1-flash-lite` — produces consensus

## Workflow

### Step 1: Gather Context

Parse $ARGUMENTS to determine:
- **Research question**: The topic or question being investigated
- **Findings source**: Where to get the raw findings from

If no explicit findings source is given:
- Check for `FINDINGS.md` in the current directory
- Check `research-output/FINDINGS.md`
- Ask the user to provide findings text directly

### Step 2: Prepare Debate Prompt

Construct the debate input with:
- The research question
- All raw findings (concatenated)
- Any specific areas of focus from the user

### Step 3: Run Debate

Call the debate MCP tool:

```
mcp__openrouter-debate__debate:
  question: "[research question]"
  findings: "[concatenated findings]"
```

Optionally override the panel with:
```
mcp__openrouter-debate__debate:
  question: "..."
  findings: "..."
  panel: '{"panelists": [...], "arbiter": {...}}'
```

### Step 4: Save and Present

Save the debate output to `{OUTPUT_DIR}/DEBATE_REPORT.md` with a summary header, then present to the user.

## Output Format

```markdown
# Multi-Model Debate Report: [Topic]

## Panel
- Critic (minimax/m2.5:free) — Critical analysis
- Synthesizer (gemini-2.0-flash) — Pattern finding
- DeepDiver (deepseek-chat) — Evidence rigor

## Consensus Points
- [What all models agreed on]

## Disputed Points
- [Where models disagreed]

## Arbiter Verdict
- [Synthesized conclusion]

## Per-Model Analyses
- [Detailed output from each model]

## Confidence Scores
- [Per-claim confidence ratings]
```

## Example Usage

```
/openrouter-debate "Is RLHF or DPO more effective for LLM alignment?" — findings: research-output/FINDINGS.md

/openrouter-debate "What are the key architectural innovations in Mamba-2?" — effort: max
```

> 💡 Parameters flow through: `— effort: max` increases tokens per model; `— panel:` customizes the model lineup.

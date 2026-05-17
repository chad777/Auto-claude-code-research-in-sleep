# Integration Contract (ARIS-Code v0.4.8+)

When one ARIS-Code skill invokes a helper script (`.py` / `.sh`), the
coupling must be **engineered**, not assumed. This document formalises
the resolver chain and failure policies that every SKILL.md must use
when calling helpers.

**Rule of thumb**: SKILL.md prose can *describe* an integration; it
cannot *guarantee* one. Any helper invocation whose silent failure
would damage the research result needs an explicit resolver block AND
an explicit failure policy from §2 (one of six: A/B/C/D1/D2/E). Prose-only "MUST invoke X" has
shipped silent-failure bugs in both ARIS branches:

- main branch (2026-04-21): community user installed via
  `install_aris.sh`, SKILL.md hardcoded `python3 tools/foo.py`,
  installer didn't propagate `tools/`, shell silent-exit-2, calling
  SKILL proceeded as if nothing happened, user's research-wiki/papers/
  stayed empty for a week
- aris-code v0.4.7: SKILL.md hardcoded `python3 tools/foo.py`, but the
  bundled-binary distribution materialised helpers to `<cwd>/<skill>/`
  not `tools/`, same silent exit 2 on every fetcher call, same silent
  cascade

## §1 Resolver chain (Layer 1 → Layer 4)

Every SKILL.md helper invocation must resolve the helper path via the
following four-layer chain. **First hit wins.** Strict-safe POSIX
(works under `set -e` / `set -u` / dash / macOS bash 3.2; uses
`${HOME:-}` so unset HOME doesn't trip `set -u`):

```bash
# BUNDLE_KEY is the resource key the SKILL author chose. Examples:
#   BUNDLE_KEY=tools/arxiv_fetch.py
#   BUNDLE_KEY=skills/paper-write/templates/IEEEtran.cls   # skill-local example
BUNDLE_KEY="tools/foo.py"
# REL is the legacy in-tree relative path; for shared cross-skill helpers
# this is the same suffix as BUNDLE_KEY ("tools/foo.py"); for skill-local
# helpers strip the "skills/<name>/" prefix (e.g. "templates/IEEEtran.cls"
# or "scripts/figure_renderer.py").
REL="${BUNDLE_KEY#skills/*/}"

# Layer 1: active skill dir — when running an exported/customised
# filesystem skill, the Skill tool's resolver preamble injects the
# absolute path as a literal (NOT via env var). The bash example below
# uses an ACTIVE_SKILL_DIR shell variable that you should populate from
# the literal path the preamble shows you ("`<active_skill_dir>/...`"),
# or leave empty for bundled-only skills.
HELPER=""
if [ -n "${ACTIVE_SKILL_DIR:-}" ] && [ -f "$ACTIVE_SKILL_DIR/$REL" ]; then
  HELPER="$ACTIVE_SKILL_DIR/$REL"
fi
# Layer 2: user-customised skill dir
[ -n "$HELPER" ] || { [ -n "${HOME:-}" ] && [ -f "$HOME/.config/aris/$BUNDLE_KEY" ] && HELPER="$HOME/.config/aris/$BUNDLE_KEY"; }
# Layer 3: ARIS-Code bundled cache (set by runtime startup)
[ -n "$HELPER" ] || { [ -n "${ARIS_CACHE_DIR:-}" ] && [ -f "$ARIS_CACHE_DIR/$BUNDLE_KEY" ] && HELPER="$ARIS_CACHE_DIR/$BUNDLE_KEY"; }
# Layer 4: project-workspace fallback (in-repo run, legacy main-branch layout)
[ -n "$HELPER" ] || { [ -f "$REL" ] && HELPER="$REL"; }
# Empty = unresolved; pick a policy from §2
```

Layer-by-layer source:

| Layer | Path | Source |
|---|---|---|
| 1 | `<active_skill_dir>/<rel>` | runtime preamble injects the literal absolute path for filesystem skills (NOT via env var); empty for bundled skills |
| 2 | `~/.config/aris/skills/<name>/<rel>` (or `~/.config/aris/<bundle-key>` for shared) | user-customised skill via `/skills export` |
| 3 | `$ARIS_CACHE_DIR/<bundle-key>` | bundled into the aris binary, materialised at startup. Works for both `tools/<rel>` and `skills/<name>/<rel>` keys |
| 4 | `<project_root>/<rel>` | in-repo run, or manual workspace copy (legacy compat) |

Each Skill tool invocation receives a `helperReport` field (JSON) in
its output that lists the actual absolute paths the bundle materialised
into AND the literal active_skill_dir. Read it; do not assume.

## §2 Failure policies — per-helper choice (six policies, A/B/C/D1/D2/E)

The resolver leaves `$HELPER=""` if all four layers miss. The caller
must choose ONE of these six policies based on how the helper
contributes to the research outcome. Mixing policies inside one SKILL
is fine; just be explicit.

| Policy | Behaviour when unresolved | Example helper |
|---|---|---|
| **A — gate** | abort with explicit ERROR; submission-blocking | `verify_paper_audits.sh` — without it the paper cannot ship |
| **B — side-effect** | warn and skip; SKILL's primary output unaffected | `research_wiki.py ingest_paper` in non-research-wiki callers |
| **C — forensic** | write the required artifacts inline per schema; never silently skip | `save_trace.sh` |
| **D1 — primary cascade** | try N sources in priority order; accept first success | `verify_papers.py` falls through to `[UNVERIFIED]` tagging |
| **D2 — multi-source aggregate** | invoke every resolved source; aggregate; fail only if zero sources contributed | `/research-lit` aggregating arXiv + Semantic Scholar + DeepXiv + Exa |
| **E — diagnostic** | exit code captured, NOT propagated as a workflow gate | `verify_wiki_coverage.sh` — informational only |

### Strict-safe wrappers

`set -e` and `set -u` aside, the helper itself may legitimately exit
non-zero (e.g. `verify_papers.py` returns non-zero when one citation
fails — caller still wants the JSON output). Use this wrapper so the
SKILL doesn't abort:

```bash
if output=$("$HELPER" "$@" 2>&1); then
  # success path
  echo "$output" | jq '.results[]'
else
  # exit-nonzero path: capture output, decide policy
  ec=$?
  case "$POLICY" in
    A) echo "ERROR: $HELPER failed (exit $ec): $output" >&2; exit 1 ;;
    B) echo "WARN: $HELPER skipped (exit $ec)" >&2 ;;
    *) printf '%s\n' "$output" ;;  # forward for caller to handle
  esac
fi
```

## §3 Per-helper policy assignment (v0.4.8 baseline)

| Bundle key | Policy | Notes |
|---|---|---|
| `tools/arxiv_fetch.py` | D2 | one source in `/research-lit` aggregate |
| `tools/deepxiv_fetch.py` | D2 | same |
| `tools/exa_search.py` | D2 | same |
| `tools/semantic_scholar_fetch.py` | D2 | same |
| `tools/openalex_fetch.py` | D2 | bundled but not yet wired into `/research-lit` source table; available for skills that opt-in |
| `tools/save_trace.sh` | C | forensic; missing = write inline trace fallback |
| `tools/verify_papers.py` | D1 | falls through to `[UNVERIFIED]` tag |
| `tools/verify_paper_audits.sh` | A | submission-blocking gate |
| `tools/research_wiki.py` | B (in callers) / A (in `/research-wiki` itself) | side-effect for `/idea-creator`/`/research-lit`/`/result-to-claim` callers; gate for `/research-wiki init` itself. **v0.4.9**: promoted from skill-local `skills/research-wiki/` to shared `tools/` namespace (9+ callers). |

Skills authored after v0.4.8 must declare their helper's policy in
SKILL.md (one line near the helper invocation) before review.

## §4 Why this matters in aris-code specifically

aris-code is a *single binary* distribution: the model receives a
SkillOutput with a resolver preamble injected by the runtime, then
issues `bash` calls. The model can drift away from the canonical
chain under context pressure. The preamble + this contract + the
per-helper policy table together close the regression class that
shipped silent-failure bugs in v0.4.7.

If you're a SKILL author: pick a policy, write the resolver block,
test the unresolved path. Don't trust prose; the executor doesn't.

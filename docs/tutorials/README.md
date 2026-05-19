# ARIS Tutorials

Long-form interview-prep cheat sheets, written in Markdown and rendered to single-file HTML via the `/render-html` skill (academic-newspaper template, sticky TOC, MathJax + highlight.js, cross-model codex review gate).

> 📖 **Curated collection**: [github.com/wanshuiyin/ARIS-in-AI-Offer](https://github.com/wanshuiyin/ARIS-in-AI-Offer) — interview-prep cheat sheets organized into 6 categories with bilingual README.

### 🧠 General / Foundations

| Tutorial | MD | HTML | Topics |
|---|---|---|---|
| **Attention 面试 Cheat Sheet** | [md](attention_tutorial.md) | [html](https://wanshuiyin.github.io/Auto-claude-code-research-in-sleep/tutorials/attention_tutorial.html) | Scaled-dot-product, MHA / MQA / GQA, RoPE / ALiBi, FlashAttention, KV cache, attention in diffusion, NaN-mask trap |

### 🏛️ LLM Architecture & Systems

| Tutorial | MD | HTML | Topics |
|---|---|---|---|
| **Long Context (RoPE / YaRN / NTK / MLA / StreamingLLM)** | [md](long_context_rope_yarn_mla_tutorial.md) | [html](https://wanshuiyin.github.io/Auto-claude-code-research-in-sleep/tutorials/long_context_rope_yarn_mla_tutorial.html) | RoPE rotation, PI/NTK/YaRN/LongRoPE scaling, MLA decoupled RoPE, SWA + StreamingLLM, Ring Attention |
| **KV Cache + Speculative Decoding** | [md](kv_cache_speculative_decoding_tutorial.md) | [html](https://wanshuiyin.github.io/Auto-claude-code-research-in-sleep/tutorials/kv_cache_speculative_decoding_tutorial.html) | PagedAttention, MQA/GQA/MLA, speculative decoding acceptance prob, Medusa / EAGLE-1/2/3, Lookahead |
| **Distributed Training** | [md](distributed_training_tutorial.md) | [html](https://wanshuiyin.github.io/Auto-claude-code-research-in-sleep/tutorials/distributed_training_tutorial.html) | DDP / FSDP2 / ZeRO 1/2/3 + ZeRO++ / TP (Megatron) / PP (GPipe, 1F1B, interleaved) / SP / CP / EP / DualPipe / Llama 3 |

### 🎯 Post-Training & Reasoning

| Tutorial | MD | HTML | Topics |
|---|---|---|---|
| **Reasoning Models (o1 / R1 / Test-Time Compute / PRM)** | [md](reasoning_models_tutorial.md) | [html](https://wanshuiyin.github.io/Auto-claude-code-research-in-sleep/tutorials/reasoning_models_tutorial.html) | o1/o3/R1 三家对比 · GRPO 推导 · PRM vs ORM · s1 budget forcing · MCTS+PUCT · R1-Distill |

### 🌊 Generative Models — Theory & Tokenizers

| Tutorial | MD | HTML | Topics |
|---|---|---|---|
| **Flow Matching Quick Reference** | [md](flow_matching_tutorial.md) | [html](https://wanshuiyin.github.io/Auto-claude-code-research-in-sleep/tutorials/flow_matching_tutorial.html) | Conditional FM, Rectified Flow / VP / VE paths, training + sampling code, ODE solvers, SD3 / FLUX latent FM |
| **Diffusion Foundations (DDPM / Score / DDIM / EDM / CFG)** | [md](diffusion_foundations_tutorial.md) | [html](https://wanshuiyin.github.io/Auto-claude-code-research-in-sleep/tutorials/diffusion_foundations_tutorial.html) | DDPM ELBO + L_simple, score matching + Tweedie, Score SDE + PF-ODE, DDIM, EDM preconditioning + Heun, CFG, Consistency Models + LCM + Turbo |

### 🎨 Generation Systems

| Tutorial | MD | HTML | Topics |
|---|---|---|---|
| **Image Generation Systems** | [md](image_generation_systems_tutorial.md) | [html](https://wanshuiyin.github.io/Auto-claude-code-research-in-sleep/tutorials/image_generation_systems_tutorial.html) | LDM · SD/SDXL/SD3/FLUX · DiT · AdaLN-Zero · ControlNet · IP-Adapter · LoRA · DreamBooth · ADD/LADD distillation |
| **Video Generation** | [md](video_generation_tutorial.md) | [html](https://wanshuiyin.github.io/Auto-claude-code-research-in-sleep/tutorials/video_generation_tutorial.html) | 3D Causal VAE · Spacetime Patches · Spatiotemporal Attention · MM-DiT · I2V · VBench · Sora / Hunyuan-Video / Wan |
| **3D Generation** | [md](3d_generation_tutorial.md) | [html](https://wanshuiyin.github.io/Auto-claude-code-research-in-sleep/tutorials/3d_generation_tutorial.html) | NeRF volumetric rendering · Instant-NGP hash · 3DGS rasterization · SDS / VSD · Trellis / Hunyuan3D |

### 👁️ Multimodal

| Tutorial | MD | HTML | Topics |
|---|---|---|---|
| **VLM (CLIP / LLaVA / Qwen-VL / DeepSeek-VL)** | [md](vlm_multimodal_tutorial.md) | [html](https://wanshuiyin.github.io/Auto-claude-code-research-in-sleep/tutorials/vlm_multimodal_tutorial.html) | CLIP InfoNCE derivation, SigLIP, ViT, BLIP-2 Q-Former, Flamingo Perceiver, LLaVA, Qwen2-VL M-RoPE |

> Additional categories (Post-Training & Reasoning · more Generation Systems) being written; full catalog at [ARIS-in-AI-Offer](https://github.com/wanshuiyin/ARIS-in-AI-Offer).

## How they were produced

The two pilots were drafted by hand and rendered via `/render-html`. Subsequent tutorials use the dedicated workflow skill:

```
/interview-cheatsheet "<TOPIC>"            # default: 600-line balanced effort
/interview-cheatsheet "<TOPIC>" — effort: max    # ~1000 lines + deeper proofs
```

`/interview-cheatsheet` ([`skills/interview-cheatsheet/SKILL.md`](../../skills/interview-cheatsheet/SKILL.md)) is an ARIS skill that:

1. Plans a 12-14 section structure (TL;DR · intuition · formula+derivation · from-scratch PyTorch · variants · 25 高频面试题 L1/L2/L3)
2. Drafts the MD following the canonical style of the two pilot tutorials (heading conventions, table-pipe escapes, callout-list separation rules — all bugs caught during the pilot reviews are now encoded into the style guide)
3. Cross-model `codex gpt-5.5 xhigh` review on math / code / interview-answer / citation correctness + personal-info redaction (fresh thread, never `codex-reply`)
4. Fix-and-loop — trajectory-based (no hard cap; stop if same issue recurs or ~6 rounds without convergence)
5. Renders via `/render-html` (which itself runs a 13-check codex review on the rendered output)
6. Writes a combined audit trail to `*.review.json`
7. **Stops — never auto-commits.** The user reviews and pushes manually.

> See [`skills/interview-cheatsheet/SKILL.md`](../../skills/interview-cheatsheet/SKILL.md) for the full skill protocol and [`skills/render-html/SKILL.md`](../../skills/render-html/SKILL.md) for the renderer.

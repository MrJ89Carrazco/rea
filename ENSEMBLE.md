# ENSEMBLE.md — The Combined REA Stack (MrJ89Carrazco forks)

This fork is the base of a three-repo reverse-engineering stack assembled for the
Ensemble council. Each repo is one layer; together they are stronger than any one
of them alone.

## The three layers

1. **ENGINE — this repo (`MrJ89Carrazco/rea`, forked from `mytechnotalent/rea`)**
   REA (Reverse Engineering Assistant), a pure-Rust framework: one command produces
   an evidence-backed Markdown/PDF/JSON report on a target file; deterministic
   offline mode with no API key, plus an autonomous AI analyst loop when a key is
   configured. It ships an **MCP server mode** (`rea serve`), which is how the
   council's coding agents drive it.

2. **KNOWLEDGE — `MrJ89Carrazco/Awesome-REA` (forked from `reacher-z/Awesome-REA`)**
   A curated, validated catalog: ~490 tools, ~206 cases, ~134 papers across six
   target domains — **Website**, Game, App, Native Binary, Firmware/Hardware,
   **Protocol** — plus an **Agents and MCP** section. For service targets (hosted
   APIs, web apps, streaming endpoints), start in Website + Protocol + Agents/MCP.

3. **PLAYBOOKS — `MrJ89Carrazco/awesome-REA-cases` (forked from
   `songshikang0111/awesome-REA-cases`)**
   842 worked cases of AI-assisted reverse engineering, categorized and refreshed
   frequently. Voice-relevant entries include a voice-input client rebuild (client
   records + reads cursor context + pastes; recognition and polishing server-side)
   and smart-speaker voice-assistant experiments.

## How the council uses this stack

- **Coding seats drive the engine.** Claude Code, Codex, and Antigravity connect to
  REA over MCP from their own terminals; chat seats (Claude, ChatGPT, Gemini,
  Copilot) plan and interpret. Nobody hand-waves: REA reports are evidence, and the
  grounding validator flags claims not backed by engine output.
- **Knowledge before brute force.** Before taking apart a new target, search the
  Awesome-REA catalog for the domain playbook and the cases repo for a worked
  example of the same shape.
- **Everything learned goes back.** New findings are written up as cases in our own
  notes (Brain first), so the stack gets smarter with every target.

## First target class: real-time voice services

Reference study already completed (ElevenLabs + Wispr Flow): the pattern both
products confirm is a thin client (capture, context, playback) with the heavy
stages — streaming ASR, LLM, streaming TTS — server-side, engineered to a latency
budget (natural conversation needs roughly <800 ms end-of-speech to first audio).
Neither upstream repo documents ElevenLabs internals; what this stack provides is
the method and tooling to analyze public client/API surfaces ourselves.

## Attribution

All three upstreams keep their own licenses (see each repo's LICENSE). This file
is an index and operating note, not a relicense: code and content remain under
their original terms, and upstream credit stays attached.

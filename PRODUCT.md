# Product

## Register

product

## Users

Developers who run multiple coding agents (Claude Code, Codex, OpenCode) in
parallel across several repositories. They organize work as
`workspace → thread → direction → session`, and at any moment they are
watching — and occasionally steering — several live agent runs at once. Their
context: a focused, often long desktop session, frequently in a dark room or
beside an IDE, switching attention between runs and wanting to know "what is
each agent doing right now" at a glance.

## Product Purpose

weft is a local-first **orchestrator** for parallel, multi-repo, multi-tool
agent work. The core is the *structure and coordination*, not the watching:

1. **Organize** — turn scattered repos into a logical workspace; a plan derives
   per-repo **scope** (write / read / none) and splits work into **directions**;
   each write-repo gets an isolated git worktree.
2. **Orchestrate** — run heterogeneous agents (Claude / Codex / OpenCode), one
   per direction, in parallel across the workspace's threads.
3. **Coordinate** — a per-thread **bus** (MCP) lets those agents talk
   (post / inbox / ask) and a coordinator wakes them, so parallel directions
   converge instead of drifting.

**Home is a conversation, not a terminal grid.** The primary surface is a **lead
agent** (the user's main chat + control tower): it reads the repos read-only,
plans, derives scope, and drives **worker** sessions per direction — it does not
write code itself. Sessions carry `role = lead | worker`. Workers report back
structured summaries + diff stat through the bus; the lead never ingests their
raw transcripts. The embedded native TUI is the **interaction surface** for a
single session, not the product's reason for being. Watching execution detail is
incidental; the value is the orchestration + coordination layer no single-agent
tool provides. Success looks like: a developer runs several agents across several
repos on one feature and weft keeps the work structured, scoped, and coordinated
— not five terminals to babysit.

It is explicitly **not** a terminal emulator, and equally not a "watch the
agents go" dashboard. It is the workspace-and-coordination fabric the agents
run inside.

## Brand Personality

Composed, exact, quietly alive. Three words: **calm, precise, native-fast.**
The voice is an expert peer's, not a vendor's: it states what is happening and
gets out of the way. No hype, no hand-holding, no decoration for its own sake.
When something is running, the interface should feel like a well-instrumented
control room — legible, everything in its place, motion only when something
actually changed. Dark by default but fully at home in light too (both are
designed, system-following + toggleable).

## Anti-references

- **Generic SaaS pastel gradients** — purple-blue gradient heroes, rounded
  card seas, emoji decoration. The template-site sweetness.
- **Heavy decoration / glassmorphism** — blur stacks, glow, big drop shadows,
  flourish animations. Showy, attention-scattering, slow.
- **Dry enterprise back-office** — gray-on-gray, dense tables with zero
  rhythm, zero craft. Usable but joyless.
- **Terminal/"matrix" aesthetic** — weft frames terminals; it is not one.
  Avoid neon-green-on-black, scanlines, faux-CRT, monospace-everything.

## Design Principles

1. **Structure is the product.** The workspace → thread → direction → scope
   model is what weft sells. Surfaces make that structure legible and editable;
   the terminal is a leaf, not the headline.
2. **Coordination over observation.** Favor what helps parallel directions
   converge — scope, plan, handoffs, the thread bus — over features that merely
   display execution. Don't build a "watch the agents go" dashboard.
3. **Frame, don't redraw.** We host native TUIs verbatim as the interaction
   surface for one session. weft's craft is the orchestration shell around them,
   never reskinning or reinterpreting agent output.
4. **Calm under parallelism.** Many threads and directions at once must read as
   composed, not busy. Density without noise; the eye always finds the one
   thing that changed.
5. **Mirror the user's tools, never override them.** weft reflects native agent
   state (permissions, sessions, config) and never invents or overrides it.
6. **Hide the mechanism, present the decisions.** worktrees / PTY / MCP bus /
   add-dir / sidecar are plumbing — they recede into Inspect. What the user owns
   stays first-class: scope, branch / PR / diff, tool choice, effective skills.
   Every abstraction ships with an escape hatch (real path / open terminal) and
   a readable failure.
7. **Needs-you first.** The kanban is an agent + git projection that flows itself;
   the human's job is exception-handling, so "what's waiting on me" is always the
   most prominent thing, aggregated workspace-wide.
8. **Bilingual from day one.** zh / en, two layers — UI strings AND agent output
   language. Never hardcode user-facing strings; internal state enums stay
   English, code/identifiers always English.

## Accessibility & Inclusion

- WCAG AA contrast on the dark surface: body text ≥ 4.5:1, large/secondary
  ≥ 3:1. No light-gray "elegance" text that fails to read.
- **Status is never color-only.** Every run state pairs color with a shape,
  icon, and/or label (running / waiting / approval / error / paused), so it
  survives color-blindness and grayscale.
- Full keyboard navigation; visible focus rings on every interactive element.
- `prefers-reduced-motion` honored everywhere — reveals and transitions degrade
  to instant or a simple crossfade.

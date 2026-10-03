# DX1 developer presentation study

Question: preserve the accepted Beta-like player UI while adding an explicit generic developer
console and diagnostic presentation. This is engine tooling, not historical chat or gameplay.

`just refs-status` checked historical reference `mc-b173-release` at `740c583901e1` and
independent `mc173` at `16f39e762da2`. Existing project F3 and depth-tested selection lines provide
the implementation-independent baseline; no historical command semantics or proprietary assets
are required. Historical console/chat implementation is not architecture to inherit.

Invariants: F3 Overview remains available; tooling never owns gameplay authority; ordinary startup
has no console/script execution; console captures input while open; text/error/history are bounded;
world-space debug geometry is transient and depth tested. Existing inventory presentation is retained.

Chosen behavior: opt-in `--devtools`, backquote console, slash commands versus persistent Rhai REPL,
F10 scenario abort. Pages use immutable bounded snapshots. Scenario waits yield every event turn;
pause stops fixed simulation only. Captures reuse the renderer. These are intentional engine
extensions, with no Beta command/chat compatibility claim. Safe deviations include key binding,
console layout, page names and diagnostic color. Avoid expensive per-frame reconstruction by
sampling snapshots at bounded cadence. Acceptance requires automated graphical/headless scenarios,
input/editing tests and failure-frame evidence, with no proprietary assets in shipped examples.

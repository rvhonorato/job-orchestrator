// job-orchestrator — slide deck
// Build: typst compile slides.typ slides.pdf
//  Dark:  typst compile slides.typ slides.pdf --input theme=dark   (default)
//  Light: typst compile slides.typ slides-light.pdf --input theme=light

#let dark = sys.inputs.at("theme", default: "light") == "dark"

// Oxocarbon dark palette (IBM Carbon dark)
#let bg-d      = rgb("#161616")
#let surface-d = rgb("#262626")
#let base02-d  = rgb("#393939")
#let comment-d = rgb("#6f6f6f")
#let fg-d      = rgb("#f2f4f8")
#let teal-d    = rgb("#08bdba")
#let cyan-d    = rgb("#33b1ff")
#let blue-d    = rgb("#78a9ff")
#let purple-d  = rgb("#be95ff")
#let pink-d    = rgb("#ff7eb6")
#let magenta-d = rgb("#ee5396")
#let green-d   = rgb("#42be65")
#let red-d     = rgb("#fa4d56")
#let orange-d  = rgb("#ff832b")

// Oxocarbon light palette (IBM Carbon light)
#let bg-l      = rgb("#ffffff")
#let surface-l = rgb("#f4f4f4")
#let base02-l  = rgb("#e0e0e0")
#let comment-l = rgb("#525252")
#let fg-l      = rgb("#161616")
#let teal-l    = rgb("#007d79")
#let cyan-l    = rgb("#0043ce")
#let blue-l    = rgb("#0f62fe")
#let purple-l  = rgb("#6929c4")
#let pink-l    = rgb("#9f1853")
#let magenta-l = rgb("#9f1853")
#let green-l   = rgb("#198038")
#let red-l     = rgb("#da1e28")
#let orange-l  = rgb("#8a3800")

// Active palette
#let bg      = if dark { bg-d }      else { bg-l }
#let surface = if dark { surface-d } else { surface-l }
#let base02  = if dark { base02-d }  else { base02-l }
#let comment = if dark { comment-d } else { comment-l }
#let fg      = if dark { fg-d }      else { fg-l }
#let teal    = if dark { teal-d }    else { teal-l }
#let cyan    = if dark { cyan-d }    else { cyan-l }
#let blue    = if dark { blue-d }    else { blue-l }
#let purple  = if dark { purple-d }  else { purple-l }
#let pink    = if dark { pink-d }    else { pink-l }
#let magenta = if dark { magenta-d } else { magenta-l }
#let green   = if dark { green-d }   else { green-l }
#let red     = if dark { red-d }     else { red-l }
#let orange  = if dark { orange-d }  else { orange-l }

// ===========================================================================
// global page settings
// ===========================================================================
#set page(
  paper:     "presentation-16-9",
  margin:    (x: 40pt, y: 32pt),
  numbering: none,
  fill:      bg,
)
#set text(font: "Noto sans", size: 15pt, fill: fg)
#show raw: set text(font: "Noto sans Mono", fill: green, size: 11pt)
#show heading.where(level: 1): it => {
  pad(x: -40pt, bottom: 12pt)[
    #block(fill: surface, width: 100%, inset: (left: 14pt, right: 40pt, y: 10pt),
      radius: (bottom-left: 6pt, bottom-right: 6pt),
    )[
      #box(width: 4pt, height: 22pt, fill: teal, radius: 2pt)
      #h(12pt)
      #text(fill: fg, size: 20pt, weight: "bold")[#it.body]
    ]
  ]
}

// ===========================================================================
// helpers
// ===========================================================================
//  define the bullet list style
#let bullet(body) = pad(left: 12pt, bottom: 2pt)[
  #text(fill: teal, weight: "bold")[▸] #h(2pt) #body
]
// define the sub-bullet list style
#let sub-bullet(body) = pad(left: 36pt, bottom: 3pt)[
  #text(fill: comment, size: 13pt)[– #body]
]
// define how code blocks should look like
#let codebox(body) = block(
  fill: surface, width: 100%, inset: 14pt, radius: 6pt, below: 0pt,
)[#body]
// define the tags
#let tag(color, body) = box(
  fill: color.transparentize(80%),
  stroke: color + 0.4pt,
  inset: (x: 7pt, y: 3pt),
  radius: 4pt,
)[#text(fill: color, size: 14pt, weight: "bold")[#body]]

// ===========================================================================
// TITLE
// ===========================================================================
#place(bottom + center)[
    #text(fill: comment, size: 13pt)[#datetime.today().display("[month repr:long] [year]")]
  ]
#align(center + horizon)[
  #text(fill: fg, size: 38pt, weight: "bold")[job-orchestrator]
  #v(8pt)
  #text(fill: teal, size: 17pt)[
    High-volume job orchestration for computational structural biology
  ]
  #v(28pt)
  #line(length: 50%, stroke: comment)
  #v(18pt)
  #text(size: 18pt)[Rodrigo V. Honorato, PhD]
  #v(4pt)
]

// ===========================================================================
// MOTIVATION
// ===========================================================================
#pagebreak()
= Motivation

#grid(
  columns: (1fr, 1fr),
  gutter: 20pt,
  [
    #align(center)[
      #text(size: 17pt)[*WeNMR* — worldwide e-Infrastructure for NMR & structural biology]
    ]

    #grid(
      columns: (1fr, 1fr),
      gutter: 8pt,
      block(fill: surface, inset: 10pt, radius: 8pt, width: 100%)[
        #align(center)[
          #set par(spacing: 8pt)
          #text(fill: cyan, size: 22pt, weight: "bold")[90k+]
          #v(2pt)
          #text(fill: comment, size: 11pt)[registered users]
        ]
      ],
      block(fill: surface, inset: 10pt, radius: 8pt, width: 100%)[
        #align(center)[
          #set par(spacing: 8pt)
          #text(fill: teal, size: 22pt, weight: "bold")[~40k]
          #v(2pt)
          #text(fill: comment, size: 11pt)[jobs / month]
        ]
      ],
    )

    #bullet[Portal exposes *research software* as web services]
    #bullet[High submission volume demands *reliable orchestration*]
    #bullet[*Fair allocation* — no single user monopolises slots]
  ],

  block(fill: surface, inset: 8pt, radius: 8pt, width: 100%)[
    #image("images/fig01.png", width: 100%)
  ],
)


// ===========================================================================
// ARCHITECTURE
// ===========================================================================
#pagebreak()
= Architecture — Single Binary, Dual Mode

#grid(
  columns: (1fr, 1fr),
  gutter: 15pt,
  block(fill: surface, inset: 14pt, radius: 8pt)[
    #align(center)[
      #text(fill: cyan, weight: "bold", size: 18pt)[Server]
    ]
    #v(5pt)
    #set text(size: 16pt)
    - REST API (`/upload`, `/download/{id}`, `/terminate/{id}`)
    - Quota enforcement & job routing
    - Persistent SQLite (job metadata)
    - Background: *sender* + *getter*
    - Auto-cleanup after grace period
  ],
  block(fill: surface, inset: 14pt, radius: 8pt)[
    #align(center)[
      #text(fill: pink, weight: "bold", size: 18pt)[Client]
    ]
    #v(5pt)
    #set text(size: 16pt)
    - Receives job payloads from server
    - Executes `run.sh` in isolated dirs
    - Ephemeral SQLite (transient state)
    - Background: *runner* + *updater* 
    - Returns results + exit codes
  ],
)

#v(10pt)
#align(center)[
  #text(fill: comment, size: 16pt)[
    `job-orchestrator server` │ `job-orchestrator client` — same binary, one CLI flag
  ]
]
#v(6pt)
#grid(
  columns: (1fr, 1fr, 1fr),
  gutter: 10pt,
  block(fill: base02, inset: 10pt, radius: 6pt, width: 100%)[
    #align(center)[
      #text(fill: teal, weight: "bold", size: 14pt)[Shared types]
      #v(2pt)
      #text(fill: comment, size: 13pt)[Structs defined once, used on both sides — no protocol drift]
    ]
  ],
  block(fill: base02, inset: 10pt, radius: 6pt, width: 100%)[
    #align(center)[
      #text(fill: teal, weight: "bold", size: 14pt)[Tight coupling]
      #v(2pt)
      #text(fill: comment, size: 13pt)[Server and client evolve together, in the same commit]
    ]
  ],
  block(fill: base02, inset: 10pt, radius: 6pt, width: 100%)[
    #align(center)[
      #text(fill: teal, weight: "bold", size: 14pt)[Single artefact]
      #v(2pt)
      #text(fill: comment, size: 13pt)[One binary to build, version, and ship across all nodes]
    ]
  ],
)

// ===========================================================================
// TECH STACK
// ===========================================================================
#pagebreak()
= Tech Stack & Key Choices

#block(fill: base02, inset: 12pt, radius: 8pt, width: 100%)[
  #grid(
    columns: (auto, 1fr),
    gutter: 14pt,
    align: horizon,
    [
      #text(fill: comment, size: 14pt)[evolved from]
      #text(fill: comment, weight: "bold", size: 17pt)[jobd] #h(0pt)
      #sym.arrow.r #h(0pt)
      #text(fill: teal, weight: "bold", size: 17pt)[job-orchestrator]
    ],
    text(fill: comment, size: 14pt)[
      A simpler HTTP-based job dispatch layer — rewritten from scratch for reliability and throughput at scale
    ],
  )
]

#v(12pt)

#grid(
  columns: (1fr, 1fr),
  gutter: 12pt,

  block(fill: orange.transparentize(88%), stroke: orange + 0.5pt, inset: 16pt, radius: 8pt, width: 100%)[
    #text(fill: orange, weight: "bold", size: 20pt)[Rust]
    #set text(size: 15pt)
    #bullet[*Speed* — no garbage collector, predictable latency under load]
    #bullet[*Reliability* — robust and reliable mission-critical component]
    #bullet[*Throughput* — async-native, hundreds of concurrent jobs with low latency]
  ],

  grid(
    columns: 1,
    row-gutter: 10pt,
    block(fill: surface, inset: 12pt, radius: 8pt, width: 100%)[
      #text(fill: teal, weight: "bold")[No external services]
      #h(6pt)
      #text(fill: comment, size: 14pt)[Self-contained binary — nothing to install or provision]
    ],
    block(fill: surface, inset: 12pt, radius: 8pt, width: 100%)[
      #text(fill: teal, weight: "bold")[SQLite]
      #h(6pt)
      #text(fill: comment, size: 14pt)[Embedded database — persistent on server, file-based on client (transient)]
    ],
    block(fill: surface, inset: 12pt, radius: 8pt, width: 100%)[
      #text(fill: teal, weight: "bold")[Async throughout]
      #h(6pt)
      #text(fill: comment, size: 14pt)[Non-blocking from HTTP layer to storage — no thread-per-job overhead]
    ],
  ),
)

// ===========================================================================
// FAIR SCHEDULING
// ===========================================================================
#pagebreak()
= Fair Scheduling — Round-Robin Quota

#grid(
  columns: (1fr, 1fr),
  gutter: 14pt,

  [
    #bullet[Jobs are grouped by *service → user* and dispatched *round-robin*]
    #sub-bullet[A user sending a large batch cannot fill all available slots]
    #sub-bullet[Every user gets a slot until the global cap is reached]
    #v(6pt)
    #bullet[Two configurable limits per service:]
    #codebox[
      ```
      SERVICE_HADDOCK_RUNS_PER_USER=5
      SERVICE_HADDOCK_MAX_RUNS=10
      ```
    ]
  ],

  [
    #block(fill: surface, inset: 14pt, radius: 8pt, width: 100%)[
      #text(fill: cyan, weight: "bold", size: 16pt)[Server — owns the registry]
      #v(6pt)
      #set text(size: 15pt)
      - Tracks all jobs in persistent SQLite
      - Enforces quotas before dispatch
      - Queued jobs survive restarts
    ]
    #v(10pt)
    #block(fill: surface, inset: 14pt, radius: 8pt, width: 100%)[
      #text(fill: pink, weight: "bold", size: 16pt)[Client — stateless executor]
      #v(6pt)
      #set text(size: 15pt)
      - No job state of its own
      - Sole responsibility: run the payload
      - Results and exit codes reported back
    ]
  ],
)


// ===========================================================================
// JOB LIFECYCLE
// ===========================================================================
#pagebreak()
= Job Lifecycle — 12-State Machine

#text(fill: comment, size: 16pt)[
  Jobs move through a well-defined set of states — from submission on the server, through execution on the client, to cleanup. Transitions are driven entirely by async background workers.
]
#set text(size: 13pt)
#table(
  columns: (auto, 1fr, auto),
  stroke: none,
  fill: (_, row) => if calc.odd(row) { base02 } else { surface },
  inset: (x: 10pt, y: 6pt),

  table.header(
    table.cell(fill: teal)[#text(fill: bg, weight: "bold")[State]],
    table.cell(fill: teal)[#text(fill: bg, weight: "bold")[Description]],
    table.cell(fill: teal)[#text(fill: bg, weight: "bold")[Transitions to]],
  ),

  text(fill: fg,      weight: "bold")[Queued],     [Job received and waiting for dispatch],                                              text(fill: comment)[Processing],
  text(fill: fg,      weight: "bold")[Processing],  [Server is sending job to a client],                                                  text(fill: comment)[Submitted],
  text(fill: fg,      weight: "bold")[Submitted],   [Job successfully sent to client, awaiting execution],                                text(fill: comment)[Prepared],
  text(fill: fg,      weight: "bold")[Prepared],    [Payload received by client, ready to execute],                                       text(fill: comment)[Running / Invalid / Failed],
  text(fill: fg,      weight: "bold")[Running],     [Client is actively executing the job],                                               text(fill: comment)[Completed / Failed / Invalid / Killed],
  text(fill: green,   weight: "bold")[Completed],   [Job finished successfully, results available],                                       text(fill: comment)[Cleaned],
  text(fill: red,     weight: "bold")[Failed],      [Job failed permanently — non-zero exit code],                                        text(fill: comment)[Cleaned],
  text(fill: orange,  weight: "bold")[Invalid],     [Job rejected — `run.sh` missing, unsafe script, or validation failure],              text(fill: comment)[Cleaned],
  text(fill: comment, weight: "bold")[Killed],      [Job was manually terminated via API],                                                text(fill: comment)[Cleaned],
  text(fill: comment, weight: "bold")[Cleaned],     [Job data removed after retention period],                                            text(fill: comment)[—],
  text(fill: comment, weight: "bold")[Locked],      [Temporarily locked during termination or dispatch],                                  text(fill: comment)[—],
  text(fill: comment, weight: "bold")[Unknown],     [Fallback when status cannot be parsed — retried on next poll cycle],                 text(fill: comment)[—],
)

#set text(size: 15pt)

// ===========================================================================
// FILE-BASED IPC
// ===========================================================================
#pagebreak()
= Design Detail — File-Based Exit Code IPC #text(fill: comment, size: 16pt, weight: "regular")[(Inter-Process Communication)]

#grid(
  columns: (1fr, 1fr),
  gutter: 20pt,

  [
    #block(fill: red.transparentize(88%), stroke: red + 0.4pt, inset: 12pt, radius: 8pt, width: 100%)[
      #text(fill: red, weight: "bold")[The problem]
      #v(4pt)
      #set text(size: 15pt)
      Jobs run as *detached bash processes* — once detached, the exit status is lost. A plain PID watch is not enough: the PID may be reused, and there is no reliable way to capture what the script returned.
    ]
    #v(10pt)
    #block(fill: green.transparentize(88%), stroke: green + 0.4pt, inset: 12pt, radius: 8pt, width: 100%)[
      #text(fill: green, weight: "bold")[The solution]
      #v(4pt)
      #set text(size: 15pt)
      A `trap` in `run.sh` writes the exit code to a file on process exit — regardless of how the process ends. The updater polls for this file every 500 ms. File *presence* means done; file *content* is the exit code.
    ]
  ],

  [
    #text(fill: comment, size: 15pt)[Required in every `run.sh`:]
    #v(6pt)
    #codebox[
      ```bash
      #!/bin/bash
      trap 'echo $? > .orchestrator.exit' EXIT

      # ... actual computation ...
      ```
    ]
    #v(12pt)
    #bullet[Works with *kill* — `SIGTERM` triggers the trap, file is written]
    #bullet[PID reuse is safe — file check takes precedence over PID liveness]
    #bullet[No blocking wait — fits naturally into the async poll loop]
    #bullet[Enforced client-side before execution — no trap, no execution]
  ],
)


// ===========================================================================
// SECURITY
// ===========================================================================
#pagebreak()
= Security

#block(fill: red.transparentize(88%), stroke: red + 0.4pt, inset: 12pt, radius: 8pt, width: 100%)[
  #text(fill: red, weight: "bold")[The core risk]
  #h(8pt)
  #text(size: 15pt)[The client executes an arbitrary bash script submitted by a remote user — this is inherently dangerous and must be treated as untrusted input.]
]
#v(10pt)

#grid(
  columns: (1fr, 1fr),
  gutter: 14pt,

  [
    #text(fill: orange, weight: "bold", size: 16pt)[Script validation — before any execution]
    #v(4pt)
    #bullet[UTF-8 validation + 20 MiB size limit — rejects binary payloads]
    #bullet[Regex blocklist: `rm -rf`, `mkfs`, `dd`, sensitive paths (`/etc/passwd`, `/proc/`, `/sys/`, `~/.ssh/`, …)]
    #bullet[Path traversal sanitisation on upload (`../` stripping)]
  ],

  [
    #text(fill: orange, weight: "bold", size: 16pt)[Container hardening — limit blast radius]
    #v(4pt)
    #bullet[Read-only root filesystem · dropped Linux capabilities]
    #bullet[`no-new-privileges` · CPU, memory, and PID limits per container]
    #bullet[Jobs run in isolated directories — no shared state between runs]
  ],
)

#v(1fr)
#block(fill: base02, stroke: comment + 0.4pt, inset: 12pt, radius: 8pt, width: 100%)[
  #text(fill: comment, size: 14pt)[
    *Note:* this is not a sandboxed execution environment. The validation layer is a safeguard, not a security boundary — payloads are expected to come from a *trusted source* (authenticated portal users, not arbitrary internet traffic).
  ]
]


// ===========================================================================
// IN PRODUCTION
// ===========================================================================
#pagebreak()
= In Production at WeNMR


#text(fill: comment, size: 15pt)[job-orchestrator is the backbone of the *WeNMR service portal* — a multi-service platform for computational structural biology.]

#v(10pt)

#grid(
  columns: (1fr, 1fr),
  gutter: 14pt,

  block(fill: surface, inset: 14pt, radius: 8pt, width: 100%)[
    #text(fill: teal, weight: "bold", size: 16pt)[Services orchestrated]
    #v(6pt)
    #set text(size: 14pt)
    #grid(
      columns: (1fr, 1fr),
      row-gutter: 6pt,
      column-gutter: 24pt,
      [#sym.bullet #h(4pt) HADDOCK3], [#sym.bullet #h(4pt) PRODIGY],
      [#sym.bullet #h(4pt) DisVis],   [#sym.bullet #h(4pt) Whiscy],
      [#sym.bullet #h(4pt) PowerFit], [#sym.bullet #h(4pt) DeepRank],
      [#sym.bullet #h(4pt) Arctic3D], [#sym.bullet #h(4pt) FANDAS],
    )
  ],

  block(fill: surface, inset: 14pt, radius: 8pt, width: 100%)[
    #text(fill: teal, weight: "bold", size: 16pt)[Deployment]
    #v(6pt)
    #set text(size: 14pt)
    #bullet[Docker Compose (current) · Kubernetes manifests ready]
    #bullet[NGINX reverse proxy + TLS termination]
    #bullet[CI via GitHub Actions with integration test suite]
  ],
)

#v(10pt)
#block(fill: purple.transparentize(88%), stroke: purple + 0.4pt, inset: 12pt, radius: 8pt, width: 100%)[
  #text(fill: purple, weight: "bold")[What's next]
  #h(8pt)
  #text(size: 15pt)[The Kubernetes manifests exist — the next step is to battle-test the full deployment under real production load across multiple nodes in a cloud environment.]
]


// ===========================================================================
// FINAL SLIDE
// ===========================================================================
#pagebreak()

#align(center + horizon)[
  #text(fill: teal, size: 26pt, weight: "bold")[job-orchestrator]
  #v(6pt)
  #text(fill: comment, size: 13pt)[A production Rust job orchestration system for high-volume scientific computing]
  #v(28pt)
  #line(length: 60%, stroke: surface + 2pt)
  #v(20pt)

  #grid(
    columns: (1fr, 1fr, 1fr, 1fr),
    gutter: 10pt,
    block(fill: surface, inset: 10pt, radius: 6pt)[
      #text(fill: comment, size: 12pt)[Architecture]
      #v(3pt)
      #text(fill: fg, size: 13pt)[Single binary · dual mode · shared types]
    ],
    block(fill: surface, inset: 10pt, radius: 6pt)[
      #text(fill: comment, size: 12pt)[Scheduling]
      #v(3pt)
      #text(fill: fg, size: 13pt)[Round-robin quota · starvation-free]
    ],
    block(fill: surface, inset: 10pt, radius: 6pt)[
      #text(fill: comment, size: 12pt)[Design]
      #v(3pt)
      #text(fill: fg, size: 13pt)[File-based IPC · 12-state machine]
    ],
    block(fill: surface, inset: 10pt, radius: 6pt)[
      #text(fill: comment, size: 12pt)[Production]
      #v(3pt)
      #text(fill: fg, size: 13pt)[8 services · 90k users · 40k jobs/month]
    ],
  )

  #v(18pt)
  #line(length: 60%, stroke: surface + 2pt)
  #v(10pt)

  #text(fill: fg, size: 18pt, weight: "bold")[Rodrigo V. Honorato, PhD]
  #v(2pt)
  #text(fill: comment, size: 15pt)[rvhonorato\@pm.me]
  #v(2pt)
  #link("https://github.com/rvhonorato/job-orchestrator")[#text(fill: purple, size: 15pt)[github.com/rvhonorato/job-orchestrator]]
  #v(1pt)
  #text(fill: comment, size: 13pt)[#datetime.today().display("[month repr:long] [year]")]
]

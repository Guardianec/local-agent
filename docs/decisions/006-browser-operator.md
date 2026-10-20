# Browser operator authority and lifetime

Status: accepted for 0.4.0, 2026-10-20.

The app-owned Playwright helper, authenticated local named pipe, Rust HTTP broker, isolated Chromium and Windows Job supervision remain the foundation. `browser_check_page` remains for deterministic scenarios. Model work uses one task-owned session across calls and model escalation.

Rust owns typed actions, origin grants, request validation, DNS/IP checks and address pinning, budgets, approvals and cleanup. The helper exposes fixed semantic operations; model strings are never executable selectors or JavaScript. Snapshot references bind a generation to retained element handles and are checked against current page structure and attachment. Stale operations return a fresh snapshot without acting. Tagged unit actions receive an additional canonical-field check because Serde otherwise ignores extra fields on them.

Navigation grants and public resource grants are separate. Additional navigation origins require native review. Resource origins can be reviewed together; those grants permit bounded anonymous GET/HEAD subresources, not navigation. Every request still passes network validation. Public destinations use standard web ports and cannot resolve to private/reserved IPs. Loopback access uses explicit local profiles and excludes model/debug/admin ports. Redirects are manually bounded and same-origin; cross-origin redirects require explicit navigation to an approved target. Request interception is not an operating-system egress firewall.

Profiles separate static public reads, public JavaScript reads, loopback reads and approved disposable loopback interaction. Public JavaScript sessions cannot use model interaction or non-read HTTP methods. Static public forms may be filled in execution tasks with JavaScript disabled. Submission captures a bounded simple form, rejects passwords/files/hidden fields/overrides, constructs the exact encoded request in Rust, obtains explicit approval, and sends once without cookies or credentials. An HTTP response is a receipt, not proof of a site's business outcome. Explicit no-submission constraints veto submission. Dynamic authenticated forms are outside the supported boundary.

Browser-only work uses an app-owned workspace represented by an additive `browserOnly` project field. Dispatch rejects file and host-command tools there regardless of model output. Existing records default to false. Browser tasks use the existing controller, history, intent and continuation systems.

Screenshots use existing image validation and DPAPI storage. Opaque references enter task records; inference hydration remains local. Observations and diagnostics are bounded and redacted. No full HTML, cookie store, request-header archive or response-body archive is retained. Offline handoff includes selected compact evidence and omits image pixels.

Approved development scripts execute in the private task copy under existing host-code consent. Their app-owned process trees live with the task and stop on termination. Vite/Next adapters receive loopback arguments; other registered scripts must honor HOST/PORT. Approved scripts are host code, not sandboxed by browser policy.

Downloads, uploads and persistent authenticated profiles remain disabled while their safe user-facing lifecycle is evaluated. No generic MCP client, desktop control, scheduling or later-roadmap capability is introduced.

Browser HOME cleanup runs after the supervised process exits and on reconciliation of interrupted tasks. Paths require a valid task UUID, pinned ordinary ancestors and an ordinary HOME directory; junctions are refused. Windows recursive removal uses Rust's [non-following, race-protected implementation](https://doc.rust-lang.org/std/fs/fn.remove_dir_all.html). Encrypted evidence remains outside HOME. Tests cover ordinary cleanup, redirected HOME refusal and interrupted-task evidence preservation. This addresses cache files observed during acceptance testing rather than claiming Chromium leaves no disk traces by itself.

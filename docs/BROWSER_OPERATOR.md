# Browser operator (0.4.0)

This describes the 0.4.0 implementation. See [the QA record](QA_0.4.0.md) for measured results and remaining limits.

## Start a task

Choose **Browser tasks** in the sidebar to work without selecting a repository. Include the complete page URL and describe a bounded request. Static public text can use `web_fetch` without installing Chromium. Browser work needs the optional component from Settings. For a website repair, open the real project and use the normal task composer so code edits stay in its private task copy.

The agent can request browser capability later when source evidence shows a need; the native approval identifies the initial address and profile. Merely including a URL does not authorize interaction. Converse tasks have no tools; Inspect blocks file edits and public submission. Execute tasks still require separate host-code and external-effect approvals.

## Profiles and actions

| Profile | Destination | JavaScript | Model interaction | Network methods |
|---|---|---|---|---|
| `public_read` | Public HTTP/HTTPS, standard ports | Off | Static field fill/select/check in Execute; separate reviewed form submission | GET/HEAD; one exact reviewed form request |
| `public_web` | Public HTTP/HTTPS, standard ports | On | Read/navigation only | GET/HEAD |
| `local_read` | Explicit loopback development origin | On | Read/navigation only | GET/HEAD |
| `local_test` | Explicit disposable loopback development origin | On | Semantic click/fill/select/check and bounded keys after approval | Approved test origin's supported HTTP methods |

Sessions support snapshots/read, title and URL in observations, diagnostics, navigate/back/forward/reload, up to four explicit tabs, and viewport screenshots. A snapshot exposes at most 40 uniquely named controls with short-lived references. Use the latest references after each action. Stale references return fresh evidence without acting. Unexpected popups, dialogs, downloads and uploads remain blocked. Pages containing password controls can be observed with those controls excluded from semantic evidence and masked in screenshots; model interaction on those pages is blocked. Screenshots are selective; explicit visual requests can trigger an initial capture, with no continuous capture.

## Authority and network policy

One session belongs to one task and remains alive across local model escalation. It has at most 30 actions, 20 helper navigations, four tabs, 300 network requests, a 28 MiB base64 response budget and a twelve-minute lifetime. Individual resources are limited to 512 KiB, observations to 24,000 bytes, waits to five seconds and redirect chains to five hops. The existing task tool/iteration/time budgets also apply.

Navigation and resource origins have separate grants. Additional navigation origins need review. Public CDN/resource origins can be reviewed as a batch; that does not allow navigating to them. Reload after granting resources. Redirects remain same-origin and bounded. Open a cross-origin final address through a reviewed navigation. Public DNS answers are checked on every request and the selected address is pinned for that request; mixed private/public answers are refused. Proxy environment settings are ignored. Local profiles refuse model/debug/admin service ports. Non-HTTP URLs, embedded credentials, sensitive inputs and unsupported methods are rejected.

WebSockets are closed, service workers are blocked, and WebRTC/WebTransport constructors are disabled. Local profiles explicitly block known Ollama, browser-debugging and Docker ports; this is not a complete inventory of administrative services. These controls govern brokered HTTP data. They are not an OS firewall: Chromium can make speculative connections, and approved project scripts execute as the Windows user. Do not run untrusted repositories.

## Forms and files

Static public forms can be prepared with harmless text. `browser_submit` captures the current simple form and asks for review of its destination, method and fields. Rust constructs and sends that exact request once. Hidden fields, file/password fields, multipart forms, submit overrides and dynamic authenticated forms are unsupported. Explicit requests not to submit veto this tool. The returned HTTP receipt does not prove a purchase, message or other business action succeeded; do not retry automatically.

Downloads and uploads are disabled. A safe file lifecycle requires destination selection, size/type handling, cleanup and visible results; arbitrary filesystem paths are not accepted as an interim solution. Public file metadata/link discovery is supported. Controlled downloads, selected-file uploads and persistent authenticated profiles are deferred to 0.4.x. The app never imports a personal Chrome/Edge profile, cookies or tokens.

## Coding workflow

Registered npm `dev`/`start` scripts can run under host-code approval in the private task copy. Vite and Next dev receive loopback host/port arguments; other scripts must honor HOST/PORT. Existing ordinary dependencies may be physically copied; packages are not installed. The task owns the process tree. Use `stop_dev_server` before restarting a non-watching server after edits. External servers are never terminated. Browser observations, code edits and checks use the same agent loop and remain reviewable before Apply.

## Privacy, vision and troubleshooting

Page text is untrusted evidence and cannot grant permissions, read protected files or disable policy. Evidence is compact and redacted; no HTML archive, cookies or all response bodies are retained. Screenshot pixels use the existing bounded image ingestion and DPAPI encrypted blobs, with at most four task images. Vision-capable local models receive images only when requested. Browser-only tasks cannot invoke project filesystem or host commands.

Activity shows the latest page and selected screenshot. Continuation keeps compact findings; a model switch preserves live session state. Stop closes the browser and app-owned server before the terminal result and removes the task's temporary browser HOME. Startup also cleans interrupted active tasks' temporary browser profiles while preserving encrypted evidence. Older completed tasks' caches are not retroactively pruned. Restart never resumes a browser or approval automatically. Offline Codex handoff includes compact selected findings and omits image pixels; review it before choosing to share.

If a reference is stale, use the fresh snapshot. If resources are blocked, review only the needed origins and reload. For oversized pages/images, use a smaller page or semantic text. If server startup fails, verify its registered recipe and HOST/PORT support or prepare a disposable server externally. If browser component hashes changed after an app update, reinstall the optional component in Settings.

# Third-party licenses

MIT covers original application code/documentation, not dependencies, runtimes or models.

Major components: Tauri (MIT/Apache-2.0), React (MIT), TypeScript (Apache-2.0), Vite (MIT), Radix UI (MIT), Zustand (MIT), Monaco Editor (MIT), Lucide (ISC), Playwright (Apache-2.0), Rust crates with individual licenses, and SQLite (public domain). Pinned lockfiles and each package's LICENSE/NOTICE are authoritative; this summary is not an exhaustive legal inventory.

The packaged optional helper retains dependency license/notice files. Separately downloaded Chromium retains its notices. WebView2, Node.js, Ollama and selected model weights are separately installed/downloaded and retain their licenses. Review official model cards before redistribution or license-sensitive use.

Static HTML retrieval uses the [html5ever tokenizer](https://github.com/servo/html5ever), licensed MIT OR Apache-2.0. Its version is pinned in Cargo.lock.

MCP uses the official rmcp Rust SDK (Apache-2.0), process-wrap (Apache-2.0 OR MIT), jsonschema (MIT), sse-stream (MIT OR Apache-2.0) and http (MIT OR Apache-2.0). Locked crate metadata and bundled license files remain authoritative.

Default references: [Fast](https://ollama.com/library/qwen3.5:4b-q8_0), [Balanced](https://ollama.com/library/qwen3.5:9b-q4_K_M), [Strong](https://ollama.com/library/qwen3.6:35b-a3b-q4_K_M). No model weights are included in source or installer.

/**
 * pi-atm — pi extension that forwards pi events to atmd for
 * monitoring in the ATM TUI.
 *
 * Symmetric counterpart to the bash `atm-hook` script that forwards
 * Claude Code hook events. This extension subscribes to the pi events
 * that drive vendor-neutral `LifecycleEvent` translation in
 * `atm-pi-adapter`, packages each event as `{event, payload, …}`, and
 * sends it to the atmd socket as `MessageType::PiEvent { data }`. The
 * daemon's connection layer parses the payload via `RawPiEvent::to_lifecycle_event()`
 * and applies the resulting LifecycleEvent to the session registry.
 *
 * Run:
 *   pi --extension /abs/path/to/pi-atm.ts
 *
 * Config (env):
 *   ATM_SOCKET   — daemon socket path (default: /tmp/atm.sock)
 *   ATM_DEBUG=1  — append debug log to /tmp/pi-atm.log
 *
 * NEVER throws or rejects — the contract is "if atmd is unavailable,
 * pi keeps working". All failures are swallowed (and optionally logged
 * when ATM_DEBUG=1).
 *
 * Tracks beads agent-tmux-manager-6dx.
 */

import * as fs from "node:fs";
import * as net from "node:net";
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";

const SOCKET = process.env.ATM_SOCKET ?? "/tmp/atm.sock";
const DEBUG = process.env.ATM_DEBUG === "1";

/**
 * pi events the atm daemon's pi adapter knows how to translate.
 *
 * Subscribing to *only* these (rather than every declared pi event)
 * keeps wire traffic low — the high-frequency events
 * (`message_update`, `before_provider_request`, `turn_start`/`turn_end`,
 * `context`) are deliberately not forwarded; the adapter's
 * `to_lifecycle_event` returns `None` for them anyway.
 */
const FORWARDED_EVENTS = [
	"session_start",
	"session_shutdown",
	"agent_start",
	"agent_end",
	"tool_call",
	"tool_execution_end",
	"model_select",
	"input",
	"session_before_compact",
	// `context` carries cumulative cost/tokens. It fires per provider
	// request; the daemon-side translator extracts only the latest
	// `usage.{cost.total, totalTokens}` so the wire payload is small
	// even though pi attaches the full conversation snapshot.
	"context",
] as const;

function logDebug(msg: string): void {
	if (!DEBUG) return;
	try {
		fs.appendFileSync("/tmp/pi-atm.log", `${new Date().toISOString()} ${msg}\n`);
	} catch {
		// Logging is best-effort.
	}
}

/**
 * Persistent socket connection to atmd for the lifetime of this pi
 * process. Lazily opened on the first `send()`; reconnects automatically
 * if dropped (e.g. atmd restarts). One handshake per connection
 * instead of one per event keeps the daemon log quiet and avoids
 * thousands of short-lived connections in long pi sessions.
 *
 * `pendingSocket` is the in-flight connection while we wait for the
 * `connect` event; events that fire during the connect race are
 * buffered in `outbox` and flushed on connect.
 */
let activeSocket: net.Socket | null = null;
let pendingSocket: net.Socket | null = null;
let reconnectScheduled = false;

/**
 * Bounded FIFO of outgoing event lines waiting for a usable socket.
 *
 * The cap exists so that an extended atmd outage in a long pi session
 * doesn't grow this array without limit. Pi context events can be
 * tens to hundreds of KB and fire frequently; an unbounded buffer
 * would let one wedged daemon eat the pi process's heap.
 *
 * Drop-oldest: when full, the *oldest* queued line is discarded so
 * the most recent state survives. Recent events are more useful for
 * "what is this session doing now" than stale ones from minutes ago,
 * and atmd's discovery scan reseeds the live registry on reconnect
 * regardless of what we managed to deliver.
 *
 * 256 is a small cap by design — at peak event rates this is seconds
 * of buffering, enough to ride out an atmd restart but not enough to
 * accumulate megabytes during a real outage.
 */
const OUTBOX_MAX = 256;
const outbox: string[] = [];
let outboxDropped = 0;

function enqueue(line: string): void {
	outbox.push(line);
	while (outbox.length > OUTBOX_MAX) {
		outbox.shift();
		outboxDropped++;
		// Log on each power-of-two boundary so chronic drops surface
		// without spamming the log on every overflow.
		if ((outboxDropped & (outboxDropped - 1)) === 0) {
			logDebug(`outbox over cap: dropped ${outboxDropped} event(s) total`);
		}
	}
}

function openSocket(): void {
	if (activeSocket || pendingSocket) return;
	if (!fs.existsSync(SOCKET)) {
		logDebug(`socket not found: ${SOCKET}`);
		return;
	}

	const sock = net.createConnection({ path: SOCKET });
	pendingSocket = sock;

	sock.on("connect", () => {
		logDebug("socket connected");
		pendingSocket = null;
		activeSocket = sock;
		// Send the protocol handshake once for this connection.
		const connectMsg = {
			protocol_version: { major: 1, minor: 0 },
			type: "connect",
			client_id: `pi-atm-${process.pid}`,
		};
		sock.write(`${JSON.stringify(connectMsg)}\n`);
		// Drain anything that queued while we were connecting.
		while (outbox.length > 0) {
			const line = outbox.shift();
			if (line !== undefined) sock.write(line);
		}
	});

	sock.on("error", (err) => {
		logDebug(`socket error: ${err.message}`);
	});

	sock.on("close", () => {
		logDebug("socket closed");
		if (pendingSocket === sock) pendingSocket = null;
		if (activeSocket === sock) activeSocket = null;
		// Don't reconnect aggressively — let the next send() trigger
		// it, with a small backoff to avoid flapping.
		if (!reconnectScheduled) {
			reconnectScheduled = true;
			setTimeout(() => {
				reconnectScheduled = false;
			}, 1000);
		}
	});

	// Daemon may keep this socket open indefinitely; no read timeout.
	// We only write; if the daemon disconnects, `close` fires.
	sock.on("data", () => {
		// We don't expect responses to fire-and-forget pi events. Drain
		// any frames the daemon sends back.
	});
}

/**
 * Sends a single PiEvent envelope to atmd.
 *
 * Fire-and-forget: pi's flow doesn't wait. If the socket is open the
 * event lands immediately; if it's mid-connect or down, the event is
 * queued in `outbox` and flushed on connect (or dropped if connect
 * fails — daemon observability is best-effort, never gate pi's flow
 * on it).
 */
function send(envelope: unknown): void {
	const dataMsg = {
		protocol_version: { major: 1, minor: 0 },
		type: "pi_event",
		data: envelope,
	};
	const line = `${JSON.stringify(dataMsg)}\n`;

	if (activeSocket) {
		try {
			activeSocket.write(line);
			return;
		} catch (e) {
			logDebug(`write threw: ${(e as Error).message}`);
			activeSocket = null;
		}
	}

	if (pendingSocket) {
		// Queue until connect fires.
		enqueue(line);
		return;
	}

	if (!reconnectScheduled) {
		enqueue(line);
		openSocket();
	}
	// If reconnect is back-off-pending, drop on the floor — the next
	// send() after the back-off clears will trigger a fresh
	// openSocket() and the bounded outbox covers buffering from then.
}

/**
 * Builds the PiEvent wire envelope from an event name and pi-emitted
 * payload. The shape matches `atm_pi_adapter::wire::RawPiEvent` so the
 * daemon-side `serde_json::from_value::<RawPiEvent>(data)` succeeds.
 */
function buildEnvelope(eventName: string, payload: unknown, sessionId: string | undefined): unknown {
	return {
		event: eventName,
		payload,
		session_id: sessionId,
		pid: process.pid,
		tmux_pane: process.env.TMUX_PANE,
	};
}

/**
 * Wraps `ctx.ui.select` so any extension that opens a permission /
 * elicitation dialog (most often pi-amplike's bash permission gate)
 * gets atm visibility for free.
 *
 * Pi's `ExtensionContext.ui` is a getter that returns the runner's
 * shared `uiContext` — a single object across all extensions. Patching
 * its `select` method affects every extension. We do this once, on the
 * first event we receive (when the runner is fully initialized).
 *
 * The patch:
 *   1. Emits `atm_needs_input_open` to atmd → session shows "Needs input"
 *   2. Awaits the original `select` (pi blocks; user picks)
 *   3. Emits `atm_needs_input_resolved` → session resumes work
 *
 * If patching fails (different pi version, future API change), we log
 * and proceed — the rest of the extension still works.
 */
function patchUiSelectOnce(
	ctx: unknown,
	getSessionId: () => string | undefined,
): void {
	const ctxAny = ctx as { ui?: { select?: unknown; __atmPatched?: boolean } };
	const ui = ctxAny.ui;
	if (!ui || typeof ui.select !== "function") {
		logDebug("patchUiSelectOnce: no ui.select to patch");
		return;
	}
	if (ui.__atmPatched) return; // already wrapped
	const originalSelect = ui.select.bind(ui) as (
		title: string,
		options: string[],
		opts?: unknown,
	) => Promise<string | undefined>;

	// Note: `getSessionId` is a getter, not a value. It is read on every
	// `select` call so synthetic events still carry the correct session
	// id even when patching happens before `currentSessionId` is set
	// (e.g. before the first `session_start` event fires).
	ui.select = async function patchedSelect(
		title: string,
		options: string[],
		opts?: unknown,
	): Promise<string | undefined> {
		logDebug(`ui.select intercepted: ${title.slice(0, 80)}`);
		void send(buildEnvelope("atm_needs_input_open", { title }, getSessionId()));
		try {
			return await originalSelect(title, options, opts);
		} finally {
			void send(buildEnvelope("atm_needs_input_resolved", {}, getSessionId()));
		}
	};
	ui.__atmPatched = true;
	logDebug("patchUiSelectOnce: ui.select wrapped");
}

export default function (pi: ExtensionAPI): void {
	logDebug(`pi-atm starting, socket=${SOCKET}`);

	// `sessionManager.currentSessionId` only becomes available after
	// `session_start` fires. Cache it from the first event we see.
	let cachedSessionId: string | undefined;
	let uiPatched = false;

	for (const eventName of FORWARDED_EVENTS) {
		// pi.on takes a string-typed name. The TS type union it expects is
		// `ExtensionEvent`; our list contains both declared events
		// (`session_start`, `agent_start`, …) and undeclared-but-real
		// (`tool_call`). Cast to bypass the closed-union check; pi accepts
		// arbitrary names at runtime.
		// biome-ignore lint/suspicious/noExplicitAny: see comment above
		pi.on(eventName as any, async (payload: unknown, ctx: unknown) => {
			try {
				logDebug(`event fired: ${eventName}`);
				if (!cachedSessionId) {
					// Best-effort session id discovery via duck-typed access.
					const ctxAny = ctx as { sessionManager?: { currentSessionId?: string } };
					cachedSessionId = ctxAny.sessionManager?.currentSessionId;
				}
				if (!uiPatched) {
					// Patch `ctx.ui.select` once, the first time we have a ctx.
					// We do it lazily because at extension-load time the runner
					// may not have its ui bound yet. Pass a getter so the patched
					// closure always reads the current `cachedSessionId`, even if
					// patching ran before the id was discovered.
					patchUiSelectOnce(ctx, () => cachedSessionId);
					uiPatched = true;
				}
				const envelope = buildEnvelope(eventName, payload, cachedSessionId);
				// Fire-and-forget: pi's flow proceeds without waiting on us.
				// The daemon is best-effort observability; if it's slow or
				// down we should never gate pi's progress on our send.
				// The internal Promise still self-resolves via timeout/close
				// so Node's event loop drains correctly.
				void send(envelope);
			} catch (e) {
				logDebug(`handler threw on ${eventName}: ${(e as Error).message}`);
			}
			// Returning undefined means we don't block or modify pi's flow.
			return undefined;
		});
	}

	logDebug(`subscribed to ${FORWARDED_EVENTS.length} events`);
}

/**
 * @atm/pi-hook — pi extension that forwards pi events to atmd for
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
 *   pi --extension /abs/path/to/atm-pi-hook.ts
 *
 * Config (env):
 *   ATM_SOCKET   — daemon socket path (default: /tmp/atm.sock)
 *   ATM_DEBUG=1  — append debug log to /tmp/atm-pi-hook.log
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
] as const;

function logDebug(msg: string): void {
	if (!DEBUG) return;
	try {
		fs.appendFileSync("/tmp/atm-pi-hook.log", `${new Date().toISOString()} ${msg}\n`);
	} catch {
		// Logging is best-effort.
	}
}

/**
 * Sends a single PiEvent envelope to atmd.
 *
 * Each call opens a new connection, writes connect+event, waits for
 * the writes to flush, then closes. Mirrors how the bash `atm-hook`
 * script behaves (each Claude hook fires a separate process that does
 * its own connect/disconnect). Each call is bounded by a hard timeout
 * so a stalled daemon never blocks pi.
 *
 * Returns a Promise that pi will `await` — ensures buffered writes
 * actually flush before pi proceeds (and especially before pi exits
 * in `--print` mode). If we returned synchronously, pi could exit
 * before the event loop drained pending socket writes.
 */
function send(envelope: unknown): Promise<void> {
	if (!fs.existsSync(SOCKET)) {
		logDebug(`socket not found: ${SOCKET}`);
		return Promise.resolve();
	}

	const connectMsg = {
		protocol_version: { major: 1, minor: 0 },
		type: "connect",
		client_id: `pi-hook-${process.pid}`,
	};
	const dataMsg = {
		protocol_version: { major: 1, minor: 0 },
		type: "pi_event",
		data: envelope,
	};

	return new Promise<void>((resolve) => {
		let done = false;
		const finish = (label: string) => {
			if (done) return;
			done = true;
			logDebug(`send finish: ${label}`);
			try {
				sock.destroy();
			} catch {
				// best-effort
			}
			resolve();
		};

		logDebug("send: opening socket");
		const sock = net.createConnection({ path: SOCKET });

		sock.on("connect", () => {
			logDebug("send: connected, writing");
			sock.write(`${JSON.stringify(connectMsg)}\n`);
			sock.write(`${JSON.stringify(dataMsg)}\n`, () => {
				logDebug("send: writes flushed, ending");
				// Both buffers flushed at this point — close gracefully.
				sock.end();
			});
		});

		sock.on("error", (err) => finish(`error ${err.message}`));
		sock.on("close", () => finish("close"));

		// Hard cap so we never block pi longer than this.
		sock.setTimeout(200, () => finish("timeout"));
	});
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

export default function (pi: ExtensionAPI): void {
	logDebug(`atm-pi-hook starting, socket=${SOCKET}`);

	// `sessionManager.currentSessionId` only becomes available after
	// `session_start` fires. Cache it from the first event we see.
	let cachedSessionId: string | undefined;

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

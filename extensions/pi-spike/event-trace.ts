/**
 * ATM × pi event-trace spike.
 *
 * Throwaway extension: subscribes to every pi event we know about and logs
 * each fire to /tmp/atm-pi-spike-<sessionId>-<startedAt>.jsonl as JSONL.
 *
 * Run:
 *   pi --extension /abs/path/to/event-trace.ts
 *
 * Goal of this spike (beads agent-tmux-manager-9dn):
 *   - Capture pi's real event vocabulary + payload shapes during a real
 *     coding session so we can design ATM's vendor-neutral LifecycleEvent
 *     enum (T6) against ground truth instead of marketing copy.
 *   - Verify whether ATM's four states (Working / Idle / NeedsInput /
 *     Stopped) are inferable from event signals alone.
 *   - Discover pi session storage layout under ~/.pi/.
 *
 * Output: see README.md for log location + how to feed findings into
 *   docs/PI_INTEGRATION.md.
 */

import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";

// All event names declared in pi's ExtensionEvent type union (dist/core/extensions/types.d.ts).
// Keeping this hard-coded so the spike has a single source-of-truth list and is trivially
// auditable — if pi adds events later, the audit ("did we miss any?") becomes obvious.
const KNOWN_EVENTS = [
	"resources_discover",
	"session_start",
	"session_before_switch",
	"session_before_fork",
	"session_before_compact",
	"session_compact",
	"session_shutdown",
	"session_before_tree",
	"session_tree",
	"context",
	"before_provider_request",
	"after_provider_response",
	"before_agent_start",
	"agent_start",
	"agent_end",
	"turn_start",
	"turn_end",
	"message_start",
	"message_update",
	"message_end",
	"tool_execution_start",
	"tool_execution_update",
	"tool_execution_end",
	"tool_call",
	"tool_result",
	"model_select",
	"user_bash",
	"input",
] as const;

const MAX_STRING = Number(process.env.ATM_SPIKE_MAX_STRING ?? 2000);
const LOG_DIR = process.env.ATM_SPIKE_LOG_DIR ?? "/tmp";

interface LogLine {
	ts: string;
	monoMs: number;
	event: string;
	payload: unknown;
	ctxSnapshot: unknown;
}

function safeJsonReplacer(_key: string, value: unknown): unknown {
	if (typeof value === "bigint") return `${value}n`;
	if (typeof value === "function") return `[fn ${value.name || "anon"}]`;
	if (value instanceof Error) return { name: value.name, message: value.message, stack: value.stack };
	if (typeof value === "string" && value.length > MAX_STRING) {
		return `${value.slice(0, MAX_STRING)}…[+${value.length - MAX_STRING} chars]`;
	}
	if (value && typeof value === "object") {
		// Strip a known set of non-serializable fields.
		const drop = new Set(["signal", "abort", "controller", "stream"]);
		if (Object.keys(value as Record<string, unknown>).some((k) => drop.has(k))) {
			const out: Record<string, unknown> = {};
			for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
				if (!drop.has(k)) out[k] = v;
			}
			return out;
		}
	}
	return value;
}

function safeStringify(value: unknown): string {
	const seen = new WeakSet();
	const cycleSafe = (key: string, v: unknown): unknown => {
		if (v && typeof v === "object") {
			if (seen.has(v as object)) return "[Circular]";
			seen.add(v as object);
		}
		return safeJsonReplacer(key, v);
	};
	try {
		return JSON.stringify(value, cycleSafe);
	} catch (e) {
		return JSON.stringify({ __serialize_error: String(e) });
	}
}

function snapshotCtx(ctx: ExtensionContext | undefined): unknown {
	if (!ctx) return null;
	const out: Record<string, unknown> = {};
	const safe = <T>(label: string, fn: () => T): void => {
		try {
			out[label] = fn();
		} catch (e) {
			out[label] = `__error: ${String(e)}`;
		}
	};
	safe("cwd", () => ctx.cwd);
	safe("hasUI", () => ctx.hasUI);
	safe("isIdle", () => ctx.isIdle());
	safe("hasPendingMessages", () => ctx.hasPendingMessages());
	safe("contextUsage", () => ctx.getContextUsage());
	safe("model", () => {
		const m = ctx.model as { id?: string; provider?: string } | undefined;
		return m ? { id: m.id, provider: m.provider } : undefined;
	});
	safe("sessionManagerKeys", () =>
		ctx.sessionManager ? Object.keys(ctx.sessionManager as unknown as object).sort() : null,
	);
	return out;
}

export default function (pi: ExtensionAPI): void {
	const startedAt = new Date();
	const startedAtIso = startedAt.toISOString().replace(/[:.]/g, "-");
	const sessionTag = `pid${process.pid}-${startedAtIso}`;
	const logPath = path.join(LOG_DIR, `atm-pi-spike-${sessionTag}.jsonl`);
	const summaryPath = path.join(LOG_DIR, `atm-pi-spike-${sessionTag}-summary.txt`);
	const startMono = performance.now();

	// One-shot header so the JSONL is self-describing.
	const header = {
		__header: true,
		schemaVersion: 1,
		startedAt: startedAt.toISOString(),
		pid: process.pid,
		platform: `${os.platform()} ${os.release()}`,
		nodeVersion: process.version,
		piHomeEnv: process.env.PI_HOME ?? null,
		cwd: process.cwd(),
		argv: process.argv,
		piPackage: (() => {
			try {
				const pkgPath = path.join(
					path.dirname(new URL(import.meta.url).pathname),
					"..",
					"..",
				);
				return { spikeRoot: pkgPath };
			} catch {
				return null;
			}
		})(),
	};

	let logFd: number | undefined;
	const counts: Record<string, number> = {};
	const firstSeen: Record<string, number> = {};
	const lastSeen: Record<string, number> = {};

	// Synchronous appends — JSONL events fire at human speed, and async
	// WriteStream buffers may not flush before process.exit (e.g. when pi
	// runs with --help or terminates abruptly).
	const writeRaw = (text: string): void => {
		try {
			if (logFd === undefined) return;
			fs.writeSync(logFd, text);
		} catch (e) {
			// eslint-disable-next-line no-console
			console.error(`[atm-pi-spike] write failed: ${String(e)}`);
		}
	};

	const open = (): void => {
		try {
			logFd = fs.openSync(logPath, "a");
			writeRaw(`${safeStringify(header)}\n`);
			// eslint-disable-next-line no-console
			console.error(`[atm-pi-spike] logging to ${logPath}`);
		} catch (e) {
			// eslint-disable-next-line no-console
			console.error(`[atm-pi-spike] failed to open ${logPath}: ${String(e)}`);
		}
	};

	const writeLine = (line: LogLine): void => {
		writeRaw(`${safeStringify(line)}\n`);
	};

	const record = (eventName: string, payload: unknown, ctx: ExtensionContext | undefined): void => {
		const monoMs = +(performance.now() - startMono).toFixed(2);
		counts[eventName] = (counts[eventName] ?? 0) + 1;
		if (firstSeen[eventName] === undefined) firstSeen[eventName] = monoMs;
		lastSeen[eventName] = monoMs;
		writeLine({
			ts: new Date().toISOString(),
			monoMs,
			event: eventName,
			payload,
			ctxSnapshot: snapshotCtx(ctx),
		});
	};

	const writeSummary = (): void => {
		try {
			const lines: string[] = [];
			lines.push("ATM × pi event-trace spike — summary");
			lines.push(`Started:  ${startedAt.toISOString()}`);
			lines.push(`Ended:    ${new Date().toISOString()}`);
			lines.push(`Duration: ${((performance.now() - startMono) / 1000).toFixed(1)}s`);
			lines.push(`Log:      ${logPath}`);
			lines.push("");
			lines.push("Event counts (in firing order):");
			const ordered = Object.keys(counts).sort((a, b) => firstSeen[a]! - firstSeen[b]!);
			for (const ev of ordered) {
				const first = (firstSeen[ev]! / 1000).toFixed(2);
				const last = (lastSeen[ev]! / 1000).toFixed(2);
				lines.push(`  ${ev.padEnd(28)} count=${String(counts[ev]).padStart(4)}  first=${first}s  last=${last}s`);
			}
			lines.push("");
			lines.push("Events declared but never observed:");
			const observed = new Set(Object.keys(counts));
			for (const ev of KNOWN_EVENTS) {
				if (!observed.has(ev)) lines.push(`  ${ev}`);
			}
			fs.writeFileSync(summaryPath, `${lines.join("\n")}\n`);
			// eslint-disable-next-line no-console
			console.error(`[atm-pi-spike] summary at ${summaryPath}`);
		} catch (e) {
			// eslint-disable-next-line no-console
			console.error(`[atm-pi-spike] summary failed: ${String(e)}`);
		}
	};

	open();

	// Subscribe to every known event. The handler signature is (event, ctx) for
	// most events but some emit only (event); destructuring handles both.
	for (const eventName of KNOWN_EVENTS) {
		try {
			pi.on(eventName as never, async (event: unknown, ctx?: ExtensionContext) => {
				record(eventName, event, ctx);
			});
		} catch (e) {
			// eslint-disable-next-line no-console
			console.error(`[atm-pi-spike] failed to subscribe to ${eventName}: ${String(e)}`);
		}
	}

	// Final summary on shutdown — registered last so it captures the shutdown
	// event itself before flushing.
	pi.on("session_shutdown" as never, async () => {
		writeSummary();
		try {
			if (logFd !== undefined) fs.closeSync(logFd);
		} catch {
			/* ignore */
		}
	});

	// Belt-and-braces — also flush on process exit in case session_shutdown
	// doesn't fire (e.g. crash, kill, --help short-circuit).
	process.once("exit", () => {
		writeSummary();
		try {
			if (logFd !== undefined) fs.closeSync(logFd);
		} catch {
			/* ignore */
		}
	});
}

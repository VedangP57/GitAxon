<script lang="ts">
/**
 * @file CommitGraph.svelte
 * @purpose GitKraken-style Canvas graph — fixed viewport canvas, redraws on scroll.
 * @architecture Canvas API (not SVG, not Pixi). Canvas fixed to viewport height.
 */
import { untrack } from "svelte";
import { get } from "svelte/store";
import {
	Virtualizer,
	observeElementRect,
	observeElementOffset,
	elementScroll,
} from "@tanstack/virtual-core";
import {
	commits,
	branches,
	tags,
	status,
	selectedCommit,
	selectCommit,
	loadMoreCommits,
	isLoading,
	rightPanelMode,
	showWip,
	loadRepo,
	currentRepo,
	createBranchFromHash,
	centerView,
	bisectState,
	startBisect,
	markBisectGood,
	markBisectBad,
	skipBisectCommit,
	resetBisect,
} from "$lib/store";
import { onMount, onDestroy } from "svelte";
import type { BranchInfo, TagInfo, LanedCommit } from "$lib/types";
import { showToast } from "$lib/toast";
import {
	cherryPick,
	revertCommit,
	resetToCommit,
	checkoutBranch,
	searchCommits,
	getRebaseTodoForRange,
} from "$lib/tauri";

let { leftPanelOpen = true }: { leftPanelOpen?: boolean } = $props();

// Layout constants (match GitKraken)
const ROW_HEIGHT = 28;
const BRANCH_COL_WIDTH = 240;
const BRANCH_COL_WIDTH_EXPANDED = 360;
const GRAPH_COL_WIDTH = 160;
const LANE_WIDTH = 22;
const LANE_OFFSET = 30;
const DOT_RADIUS = 3.5;
const LINE_WIDTH = 1.5;

const LANE_CSS_VARS = [
	"--accent-blue",
	"--accent-green",
	"--accent-purple",
	"--accent-orange",
	"--accent-red",
	"--accent-yellow",
	"--accent-teal",
	"--accent-pink",
];
const LANE_FALLBACKS = [
	"#58a6ff",
	"#3fb950",
	"#bc8cff",
	"#f0883e",
	"#f85149",
	"#d29922",
	"#39d353",
	"#ff7b72",
];

let laneColorCache: string[] = [];

function initLaneColors() {
	laneColorCache = [0, 1, 2, 3, 4, 5, 6, 7].map(getLaneColor);
}

function getLaneColor(colorIndex: number): string {
	const idx = colorIndex % 8;
	const raw = getComputedStyle(document.documentElement)
		.getPropertyValue(LANE_CSS_VARS[idx])
		.trim();
	if (raw && raw !== "" && raw !== "#ffffff") return raw;
	return LANE_FALLBACKS[idx];
}

function laneX(lane: number): number {
	return LANE_OFFSET + lane * LANE_WIDTH;
}

function formatRelativeTime(timestamp: number): string {
	const sec = Math.floor((Date.now() - timestamp * 1000) / 1000);
	if (sec < 60) return "just now";
	if (sec < 3600) return `${Math.floor(sec / 60)} min ago`;
	if (sec < 86400) return `${Math.floor(sec / 3600)} hours ago`;
	if (sec < 2592000) return `${Math.floor(sec / 86400)} days ago`;
	if (sec < 31536000) return `${Math.floor(sec / 2592000)} months ago`;
	return `${Math.floor(sec / 31536000)} years ago`;
}

function formatTooltipDate(timestamp: number): string {
	const d = new Date(timestamp * 1000);
	const date = d.toLocaleDateString(undefined, {
		month: "long",
		day: "numeric",
		year: "numeric",
	});
	const time = d.toLocaleTimeString(undefined, {
		hour: "numeric",
		minute: "2-digit",
		hour12: true,
	});
	return `${date} at ${time}`;
}

function getDeduplicatedLabels(lc: LanedCommit): BranchInfo[] {
	const labels = branchByHash.get(lc.commit.hash) ?? [];

	const branchInfo = labels as (BranchInfo & { isTag?: boolean })[];
	const localNames = new Set(
		branchInfo
			.filter((l) => !l.isRemote && !l.isTag)
			.map((l) => l.name),
	);

	const deduplicated = branchInfo.filter((l) => {
		if (!l.isRemote) return true;
		if (l.isTag) return true;
		const baseName = l.name.split("/").slice(1).join("/");
		return !localNames.has(baseName);
	});

	return [...deduplicated].sort((a, b) => {
		if (a.isHead) return -1;
		if (b.isHead) return 1;
		if (!a.isRemote && !a.isTag) return -1;
		if (!b.isRemote && !b.isTag) return 1;
		if (a.isTag && !b.isTag) return 1;
		if (!a.isTag && b.isTag) return -1;
		return 0;
	});
}

function visibleLabels(lc: LanedCommit): BranchInfo[] {
	return getDeduplicatedLabels(lc).slice(0, 2);
}

function overflowCount(lc: LanedCommit): number {
	return Math.max(0, getDeduplicatedLabels(lc).length - 2);
}

function hiddenLabels(lc: LanedCommit): BranchInfo[] {
	return getDeduplicatedLabels(lc).slice(2);
}

// Canvas refs
let containerRef = $state<HTMLDivElement | null>(null);
let canvasEl = $state<HTMLCanvasElement | null>(null);
let ctx = $state<CanvasRenderingContext2D | null>(null);
let graphScrollWrapRef = $state<HTMLDivElement | null>(null);

let virtualizer = $state<Virtualizer<HTMLDivElement, HTMLDivElement> | null>(
	null,
);
let virtualizerVersion = $state(0);
let tooltipCommit = $state<LanedCommit | null>(null);
let tooltipPos = $state({ x: 0, y: 0 });
let searchQuery = $state("");
let filterOpen = $state(false);
let filterAuthor = $state("");
let filterSince = $state("");
let filterUntil = $state("");
let filterPath = $state("");
let isSearching = $state(false);
let searchResults = $state<LanedCommit[] | null>(null);
let bisectBadPending = $state<string | null>(null); // Hash of the bad commit waiting for a good commit

const hasActiveFilters = $derived(
	filterAuthor.trim() !== "" ||
	filterSince !== "" ||
	filterUntil !== "" ||
	filterPath.trim() !== ""
);

async function applyFilters() {
	const repo = get(currentRepo);
	if (!repo) return;
	if (!hasActiveFilters && !searchQuery.trim()) {
		searchResults = null;
		return;
	}
	isSearching = true;
	try {
		const results = await searchCommits(repo, {
			query: searchQuery.trim() || undefined,
			author: filterAuthor.trim() || undefined,
			since: filterSince || undefined,
			until: filterUntil || undefined,
			path: filterPath.trim() || undefined,
			limit: 500,
		});
		searchResults = results;
	} catch (e) {
		showToast(String(e), 'error');
	} finally {
		isSearching = false;
	}
}

function clearFilters() {
	filterAuthor = "";
	filterSince = "";
	filterUntil = "";
	filterPath = "";
	searchResults = null;
	filterOpen = false;
}

let scrollThrottled = false;
let delayedLoading = $state(false);
let loadingTimer: ReturnType<typeof setTimeout> | null = null;
let branchClickTimer: ReturnType<typeof setTimeout> | null = null;
let ctxMenu = $state<{
	x: number;
	y: number;
	commit: LanedCommit;
} | null>(null);

const totalChanges = $derived($status.length);
const branchColWidth = $derived(
	leftPanelOpen ? BRANCH_COL_WIDTH : BRANCH_COL_WIDTH_EXPANDED,
);

const commitList = $derived.by(() => {
	// If server-side search results are active, use them
	if (searchResults !== null) return searchResults;
	const all = $commits;
	const q = searchQuery.trim().toLowerCase();
	if (!q) return all;
	// Client-side filter for quick text search (no advanced filters)
	return all.filter(
		(lc) =>
			lc.commit.message.toLowerCase().includes(q) ||
			lc.commit.author_name.toLowerCase().includes(q) ||
			lc.commit.hash.toLowerCase().includes(q) ||
			lc.commit.short_hash.toLowerCase().includes(q),
	);
});
const branchList = $derived($branches);
const tagList = $derived($tags);
const selected = $derived($selectedCommit);

const rowCount = $derived(commitList.length + 1);
const totalHeight = $derived(rowCount * ROW_HEIGHT);

const branchByHash = $derived.by(() => {
	const map = new Map<string, BranchInfo[]>();
	for (const b of branchList) {
		const existing = map.get(b.tipHash) ?? [];
		existing.push(b);
		map.set(b.tipHash, existing);
	}
	return map;
});

const tagByHash = $derived.by(() => {
	const map = new Map<string, TagInfo[]>();
	for (const t of tagList) {
		const existing = map.get(t.hash) ?? [];
		existing.push(t);
		map.set(t.hash, existing);
	}
	return map;
});

const branchColorMap = $derived.by(() => {
	const map = new Map<string, string>();
	for (const lc of commitList) {
		const color = getLaneColor(lc.color_index);
		const labels = branchByHash.get(lc.commit.hash) ?? [];
		for (const label of labels) {
			map.set(label.name, color);
		}
	}
	return map;
});


function resizeCanvas() {
	if (!canvasEl || !containerRef) return;
	const dpr = window.devicePixelRatio || 1;
	const wrap = graphScrollWrapRef ?? containerRef.parentElement;
	if (!wrap) return;

	const logicalW = GRAPH_COL_WIDTH;
	const logicalH = wrap.clientHeight;

	canvasEl.width = logicalW * dpr;
	canvasEl.height = logicalH * dpr;
	canvasEl.style.width = logicalW + "px";
	canvasEl.style.height = logicalH + "px";

	const c = canvasEl.getContext("2d");
	if (c) {
		ctx = c;
		c.scale(dpr, dpr);
	}
}

function drawWipDot(scrollTop: number) {
	if (!ctx || !canvasEl) return;
	const logicalH = canvasEl.height / (window.devicePixelRatio || 1);
	const wipY = ROW_HEIGHT / 2 - scrollTop;
	if (wipY < -ROW_HEIGHT || wipY > logicalH + ROW_HEIGHT) return;

	const x = laneX(0);
	ctx.beginPath();
	ctx.setLineDash([3, 2]);
	ctx.arc(x, wipY, DOT_RADIUS + 1, 0, Math.PI * 2);
	ctx.strokeStyle =
		getComputedStyle(document.documentElement)
			.getPropertyValue("--text-secondary")
			.trim() || "#8b949e";
	ctx.lineWidth = 1.5;
	ctx.stroke();
	ctx.setLineDash([]);
}

function drawCommitRow(i: number, rowY: number) {
	const lc = commitList[i];
	if (!lc || !ctx) return;

	const cy = rowY + ROW_HEIGHT / 2;
	const bgColor =
		getComputedStyle(document.documentElement)
			.getPropertyValue("--bg-primary")
			.trim() || "#0d1117";

	// Lanes active above this row (segment between row i-1 and row i)
	const aboveMap = new Map<number, number>(
		i > 0 ? commitList[i - 1].through_lanes : [],
	);
	// Lanes active below this row (segment between row i and row i+1)
	const belowMap = new Map<number, number>(lc.through_lanes);

	// Lanes that are arc destinations at this row: they just started here via an arc
	// and must NOT draw a vertical segment (the arc itself is the visual connection).
	const arcDestLanes = new Set<number>(
		lc.edges
			.filter(e => e.from_lane !== e.to_lane && e.edge_type !== "Straight")
			.map(e => e.to_lane)
	);

	// All lanes that need any vertical segment at this row
	const allLanes = new Set<number>([
		...aboveMap.keys(),
		...belowMap.keys(),
		lc.lane,
	]);

	// 1. Vertical lane segments
	for (const lane of allLanes) {
		const hasAbove = aboveMap.has(lane);
		const hasBelow = belowMap.has(lane);
		const isCommitLane = lane === lc.lane;

		// Arc-destination lanes: no vertical segment here — the arc provides the connection.
		// Exception: if the lane was already active above (pass-through), draw the incoming part.
		if (!isCommitLane && !hasAbove && arcDestLanes.has(lane)) continue;

		let y1: number, y2: number;

		if (isCommitLane) {
			y1 = hasAbove ? rowY : cy;
			y2 = hasBelow ? rowY + ROW_HEIGHT : cy;
		} else if (hasAbove && hasBelow) {
			y1 = rowY;
			y2 = rowY + ROW_HEIGHT;
		} else if (hasAbove) {
			// Lane terminates here (arc merges it into another lane)
			y1 = rowY;
			y2 = cy;
		} else {
			// Genuinely new lane from cy (shouldn't reach here for arc dests — guarded above)
			y1 = cy;
			y2 = rowY + ROW_HEIGHT;
		}

		if (y1 >= y2) continue;

		const colorIdx = belowMap.get(lane) ?? aboveMap.get(lane) ?? lc.color_index;
		const color = laneColorCache[colorIdx % 8] ?? getLaneColor(colorIdx);

		const x = laneX(lane);
		ctx.beginPath();
		ctx.moveTo(x, y1);
		ctx.lineTo(x, y2);
		ctx.strokeStyle = color;
		ctx.lineWidth = LINE_WIDTH;
		ctx.lineCap = "round";
		ctx.stroke();
	}

	// 2. Arc edges — straight down then hook-curve at bottom (GitKraken style)
	for (const edge of lc.edges) {
		if (edge.from_lane === edge.to_lane) continue;
		if (edge.edge_type === "Straight") continue;

		const x1 = laneX(edge.from_lane);
		const x2 = laneX(edge.to_lane);
		const yStart = cy;
		const yEnd = rowY + ROW_HEIGHT;
		// Radius scales with lane distance but is capped so there's always a visible straight section
		const r = Math.min(Math.abs(x2 - x1) * 0.45, ROW_HEIGHT * 0.4);

		const color = laneColorCache[edge.color_index % 8] ?? getLaneColor(edge.color_index);

		ctx.beginPath();
		ctx.strokeStyle = color;
		ctx.lineWidth = LINE_WIDTH;
		ctx.lineCap = "round";
		// Straight down from commit center, then hook-curve only at the very bottom
		ctx.moveTo(x1, yStart);
		ctx.lineTo(x1, yEnd - r);
		ctx.quadraticCurveTo(x1, yEnd, x2, yEnd);
		ctx.stroke();
	}

	// 3. Commit dot
	const isMerge = (lc.commit.parent_hashes?.length ?? 0) > 1;
	const dotR = isMerge ? DOT_RADIUS + 1.5 : DOT_RADIUS;
	const dotX = laneX(lc.lane);
	const dotColor = laneColorCache[lc.color_index % 8] ?? getLaneColor(lc.color_index);

	ctx.beginPath();
	ctx.arc(dotX, cy, dotR + 0.5, 0, Math.PI * 2);
	ctx.fillStyle = bgColor;
	ctx.fill();

	ctx.beginPath();
	ctx.arc(dotX, cy, dotR, 0, Math.PI * 2);
	ctx.fillStyle = dotColor;
	ctx.fill();
}

function drawVisibleGraph() {
	if (!ctx || !canvasEl || commitList.length === 0) return;

	const scrollTop = containerRef?.scrollTop ?? 0;
	const dpr = window.devicePixelRatio || 1;
	const logicalW = canvasEl.width / dpr;
	const logicalH = canvasEl.height / dpr;

	const firstVisible = Math.max(
		0,
		Math.floor((scrollTop - 2 * ROW_HEIGHT) / ROW_HEIGHT),
	);
	const lastVisible = Math.min(
		commitList.length - 1,
		Math.ceil((scrollTop + logicalH + 2 * ROW_HEIGHT) / ROW_HEIGHT) - 1,
	);

	ctx.clearRect(0, 0, logicalW, logicalH);

	drawWipDot(scrollTop);

	for (let i = firstVisible; i <= lastVisible; i++) {
		const absY = ROW_HEIGHT + i * ROW_HEIGHT;
		const rowY = absY - scrollTop;
		if (rowY < -ROW_HEIGHT || rowY > logicalH + ROW_HEIGHT) continue;
		drawCommitRow(i, rowY);
	}
}

function onScroll() {
	drawVisibleGraph();
	checkInfiniteScroll();
}

function checkInfiniteScroll() {
	if (scrollThrottled) return;
	scrollThrottled = true;
	setTimeout(() => {
		scrollThrottled = false;
		const el = containerRef;
		if (!el) return;
		const { scrollTop, scrollHeight, clientHeight } = el;
		if (scrollHeight - scrollTop - clientHeight < 800) {
			loadMoreCommits();
		}
	}, 150);
}

function handleCanvasClick(e: MouseEvent) {
	if (!canvasEl || !containerRef) return;
	const rect = canvasEl.getBoundingClientRect();
	const mouseX = e.clientX - rect.left;
	const mouseY = e.clientY - rect.top;
	const scrollTop = containerRef.scrollTop;

	const absoluteY = mouseY + scrollTop;

	for (let i = 0; i < commitList.length; i++) {
		const lc = commitList[i];
		const dotX = laneX(lc.lane);
		const dotY = ROW_HEIGHT + i * ROW_HEIGHT + ROW_HEIGHT / 2;
		const dotR = DOT_RADIUS + 4;

		const dist = Math.sqrt(
			(mouseX - dotX) ** 2 + (absoluteY - dotY) ** 2,
		);

		if (dist <= dotR) {
			selectCommit(lc);
			return;
		}
	}
}

function handleGraphWheel(e: WheelEvent) {
	const el = containerRef;
	if (!el) return;
	e.preventDefault();
	el.scrollTop += e.deltaY;
}

function handleWipClick() {
	showWip();
}

async function handleCherryPick(lc: LanedCommit) {
	ctxMenu = null;
	try {
		const repo = get(currentRepo);
		if (!repo) {
			showToast("No repository open", "error");
			return;
		}
		const msg = await cherryPick(repo, lc.commit.hash);
		await loadRepo(repo);
		showToast(msg, "success");
	} catch (e) {
		showToast(String(e), "error");
	}
}

async function handleRevert(lc: LanedCommit) {
	ctxMenu = null;
	try {
		const repo = get(currentRepo);
		if (!repo) {
			showToast("No repository open", "error");
			return;
		}
		const msg = await revertCommit(repo, lc.commit.hash);
		await loadRepo(repo);
		showToast(msg, "success");
	} catch (e) {
		showToast(String(e), "error");
	}
}

async function handleReset(lc: LanedCommit, mode: "soft" | "mixed" | "hard") {
	ctxMenu = null;
	if (mode === "hard") {
		if (!confirm(`Hard reset to ${lc.commit.short_hash}? This cannot be undone.`)) return;
	}
	try {
		const repo = get(currentRepo);
		if (!repo) {
			showToast("No repository open", "error");
			return;
		}
		const msg = await resetToCommit(repo, lc.commit.hash, mode);
		await loadRepo(repo);
		showToast(msg, "success");
	} catch (e) {
		showToast(String(e), "error");
	}
}

async function handleCreateBranchHere(lc: LanedCommit) {
	ctxMenu = null;
	createBranchFromHash.set(lc.commit.hash);
}

async function handleBisectBad(lc: LanedCommit) {
	ctxMenu = null;
	bisectBadPending = lc.commit.hash;
	showToast(`Bisect: marked ${lc.commit.short_hash} as bad. Now right-click a known good commit.`, 'info');
}

async function handleBisectGood(lc: LanedCommit) {
	ctxMenu = null;
	if (!bisectBadPending) {
		showToast('Start bisect by marking a bad commit first', 'error');
		return;
	}
	try {
		await startBisect(bisectBadPending, lc.commit.hash);
		bisectBadPending = null;
		showToast('Bisect started', 'success');
	} catch (e) {
		showToast(String(e), 'error');
	}
}

async function handleRebaseOnto(lc: LanedCommit) {
	ctxMenu = null;
	// Switch to rebase view — the RebasePanel will load the todo list
	centerView.set('rebase');
	// We need to notify the RebasePanel of the target hash.
	// Dispatch a custom event that the RebasePanel can listen for.
	window.dispatchEvent(new CustomEvent('gitaxon-rebase-setup', { detail: { hash: lc.commit.hash } }));
}

async function handleCopyHash(lc: LanedCommit) {
	ctxMenu = null;
	await navigator.clipboard.writeText(lc.commit.hash);
	showToast("Copied commit hash", "info");
}

async function handleCopyMessage(lc: LanedCommit) {
	ctxMenu = null;
	await navigator.clipboard.writeText(lc.commit.message);
	showToast("Copied commit message", "info");
}

async function handleBranchDoubleClick(label: BranchInfo) {
	const repo = get(currentRepo);
	if (!repo) {
		showToast("No repository open", "error");
		return;
	}
	if (label.isTag) {
		showToast("Cannot checkout a tag from branch labels", "info");
		return;
	}
	if (label.isRemote) {
		showToast("Cannot checkout a remote branch directly", "info");
		return;
	}
	if (label.isHead) {
		showToast(`Already on ${label.name}`, "info");
		return;
	}

	try {
		await checkoutBranch(repo, label.name);
		await loadRepo(repo);
		showToast(`Switched to branch ${label.name}`, "success");
	} catch (err) {
		const msg = err instanceof Error ? err.message : String(err);
		if (msg.toLowerCase().includes("uncommitted changes")) {
			showToast(
				"Cannot checkout: you have uncommitted changes. Stage or stash them first.",
				"error",
			);
			return;
		}
		showToast(msg, "error");
	}
}

function handleBranchPillClick(e: MouseEvent, lc: LanedCommit, label: BranchInfo) {
	e.stopPropagation();

	// Use timed click handling so single-click selection doesn't cancel
	// branch checkout behavior on the second click.
	if (branchClickTimer) {
		clearTimeout(branchClickTimer);
		branchClickTimer = null;
		void handleBranchDoubleClick(label);
		return;
	}

	branchClickTimer = setTimeout(() => {
		branchClickTimer = null;
		selectCommit(lc);
	}, 220);
}

let resizeObserver: ResizeObserver | null = null;

onMount(() => {
	initLaneColors();
	if (canvasEl) {
		ctx = canvasEl.getContext("2d");
		resizeCanvas();
		drawVisibleGraph();
		canvasEl.addEventListener("click", handleCanvasClick);
	}

	const onKeyDown = (e: KeyboardEvent) => {
		if (e.key === "Escape") ctxMenu = null;
	};
	window.addEventListener("keydown", onKeyDown);
	return () => {
		window.removeEventListener("keydown", onKeyDown);
	};
});

$effect(() => {
	const wrap = graphScrollWrapRef;
	if (!wrap) return;
	resizeObserver?.disconnect();
	resizeObserver = new ResizeObserver(() => {
		resizeCanvas();
		drawVisibleGraph();
	});
	resizeObserver.observe(wrap);
	return () => resizeObserver?.disconnect();
});

$effect(() => {
	const wrap = graphScrollWrapRef;
	const scrollEl = containerRef;
	if (!wrap || !scrollEl) return;
	const onWheel = (e: WheelEvent) => {
		e.preventDefault();
		scrollEl.scrollTop += e.deltaY;
	};
	wrap.addEventListener("wheel", onWheel, { passive: false });
	return () => wrap.removeEventListener("wheel", onWheel);
});

onDestroy(() => {
	resizeObserver?.disconnect();
	canvasEl?.removeEventListener("click", handleCanvasClick);
	if (loadingTimer) clearTimeout(loadingTimer);
	if (branchClickTimer) clearTimeout(branchClickTimer);
});

$effect(() => {
	const loading = $isLoading;
	if (loading) {
		if (loadingTimer) clearTimeout(loadingTimer);
		loadingTimer = setTimeout(() => {
			delayedLoading = true;
		}, 220);
		return;
	}

	if (loadingTimer) {
		clearTimeout(loadingTimer);
		loadingTimer = null;
	}
	delayedLoading = false;
});

$effect(() => {
	const _ = commitList;
	untrack(() => {
		if (ctx && canvasEl) {
			initLaneColors();
			resizeCanvas();
			drawVisibleGraph();
		}
	});
});

$effect(() => {
	const el = containerRef;
	if (!el) return;
	el.addEventListener("scroll", onScroll, { passive: true });
	return () => el.removeEventListener("scroll", onScroll);
});

$effect(() => {
	const c = canvasEl;
	if (!c) return;
	ctx = c.getContext("2d");
	if (ctx) {
		resizeCanvas();
		drawVisibleGraph();
	}
});

$effect(() => {
	const el = containerRef;
	const list = commitList;
	if (!el) return;

	const count = list.length + 1;
	let v = virtualizer;
	if (!v) {
		v = new Virtualizer({
			count,
			getScrollElement: () => containerRef,
			estimateSize: () => ROW_HEIGHT,
			scrollToFn: (offset, opts, instance) => {
				elementScroll(offset, opts, instance);
			},
			observeElementRect,
			observeElementOffset,
			overscan: 10,
			onChange: () => {
				queueMicrotask(() => virtualizerVersion++);
			},
		});
		virtualizer = v;
		v._willUpdate();
	} else {
		v.setOptions({
			count,
			getScrollElement: () => containerRef,
			estimateSize: () => ROW_HEIGHT,
			scrollToFn: (offset, opts, instance) => {
				elementScroll(offset, opts, instance);
			},
			observeElementRect,
			observeElementOffset,
			overscan: 10,
			onChange: () => {
				queueMicrotask(() => virtualizerVersion++);
			},
		});
		v._willUpdate();
	}
});

$effect(() => {
	const v = virtualizer;
	const el = containerRef;
	if (!v || !el) return;
	const id = requestAnimationFrame(() => {
		if (el.clientHeight > 0) v.measure();
	});
	return () => cancelAnimationFrame(id);
});

$effect(() => {
	const el = containerRef;
	if (!el) return;
	const ro = new ResizeObserver(() => virtualizer?.measure());
	ro.observe(el);
	return () => ro.disconnect();
});

const virtualItems = $derived.by(() => {
	virtualizerVersion;
	const items = virtualizer?.getVirtualItems() ?? [];
	if (items.length === 0 && rowCount > 0) {
		return Array.from({ length: rowCount }, (_, i) => ({
			key: i,
			index: i,
			start: i * ROW_HEIGHT,
			end: (i + 1) * ROW_HEIGHT,
			size: ROW_HEIGHT,
		}));
	}
	return items;
});
</script>

<div
	class="graph-root"
	style="--branch-col-width: {branchColWidth}px; --graph-col-width: {GRAPH_COL_WIDTH}px;"
>
	<div class="graph-header">
		<span class="header-count">Commits: {commitList.length}</span>
		<div class="search-box">
			<input
				type="text"
				class="search-input"
				placeholder="Search commits..."
				bind:value={searchQuery}
			/>
			{#if searchQuery}
				<button
					class="search-clear"
					onclick={() => { searchQuery = ""; searchResults = null; }}
					title="Clear search"
					type="button"
				>
					×
				</button>
			{/if}
			<button
				class="filter-btn"
				class:active={hasActiveFilters || filterOpen}
				onclick={() => (filterOpen = !filterOpen)}
				title="Advanced filters"
				type="button"
			>
				⚙{#if hasActiveFilters}<span class="filter-badge"></span>{/if}
			</button>
		</div>
		{#if filterOpen}
			<div class="filter-panel">
				<div class="filter-row">
					<label class="filter-label">Author<input class="filter-input" type="text" placeholder="Author name..." bind:value={filterAuthor} /></label>
					<label class="filter-label">File<input class="filter-input" type="text" placeholder="path/to/file" bind:value={filterPath} /></label>
				</div>
				<div class="filter-row">
					<label class="filter-label">Since<input class="filter-input" type="date" bind:value={filterSince} /></label>
					<label class="filter-label">Until<input class="filter-input" type="date" bind:value={filterUntil} /></label>
				</div>
				<div class="filter-actions">
					<button class="filter-apply" onclick={applyFilters} disabled={isSearching}>
						{isSearching ? 'Searching...' : 'Apply Filters'}
					</button>
					{#if hasActiveFilters}
						<button class="filter-clear" onclick={clearFilters}>Clear</button>
					{/if}
				</div>
			</div>
		{/if}
	</div>

	{#if $bisectState?.active}
		<div class="bisect-bar">
			<span class="bisect-icon">🔍</span>
			<span class="bisect-text">BISECT: testing {$bisectState.current_short_hash}</span>
			{#if $bisectState.steps_remaining != null}
				<span class="bisect-steps">~{$bisectState.steps_remaining} steps left</span>
			{/if}
			{#if $bisectState.found_hash}
				<span class="bisect-found">Found: {$bisectState.found_hash.slice(0, 7)}</span>
			{/if}
			<div class="bisect-actions">
				<button class="bisect-btn good" onclick={markBisectGood}>Good</button>
				<button class="bisect-btn bad" onclick={markBisectBad}>Bad</button>
				<button class="bisect-btn skip" onclick={skipBisectCommit}>Skip</button>
				<button class="bisect-btn reset" onclick={resetBisect}>End Bisect</button>
			</div>
		</div>
	{/if}

	{#if delayedLoading}
		<div class="graph-state graph-loading">
			<div class="graph-spinner" aria-label="Loading"></div>
			<div class="graph-loading-text">Loading repository...</div>
		</div>
	{:else if commitList.length === 0}
		<div class="graph-state graph-empty">
			<div class="empty-icon">∅</div>
			<div class="empty-text">No commits found</div>
		</div>
	{:else}
		<div class="graph-headers">
			<div class="gh-branch">BRANCH / TAG</div>
			<div class="gh-graph">GRAPH</div>
			<div class="gh-message">COMMIT MESSAGE</div>
		</div>

		<div class="graph-scroll-wrap" bind:this={graphScrollWrapRef}>
			<div
				class="graph-scroll"
				bind:this={containerRef}
				role="list"
			>
				<div
					class="scroll-content"
					style="height: {totalHeight}px; position: relative;"
				>
					{#each virtualItems as vRow (vRow.key)}
						{#if vRow.index === 0}
							<!-- svelte-ignore a11y_no_noninteractive_element_interactions a11y_click_events_have_key_events -->
							<div
								class="row wip-row"
								class:wip-clean={$status.length === 0}
								class:selected={$rightPanelMode === "wip" && !$selectedCommit}
								style="position: absolute; top: {vRow.start}px; left: 0; right: 0; height: {ROW_HEIGHT}px;"
								role="button"
								tabindex="0"
								onclick={handleWipClick}
								onkeydown={(e) =>
									(e.key === "Enter" || e.key === " ") &&
									(e.preventDefault(), handleWipClick())}
							>
								<div class="col-branch"></div>
								<div class="col-graph"></div>
								<div class="col-message labels-and-message">
									<span class="wip-label">// WIP</span>
									{#if totalChanges > 0}
										<span class="wip-count-badge">{totalChanges}</span>
									{:else}
										<span class="wip-clean">No local changes</span>
									{/if}
								</div>
								<span class="meta"></span>
							</div>
						{:else}
							{@const idx = vRow.index - 1}
							{@const lc = commitList[idx]}
							<!-- svelte-ignore a11y_no_noninteractive_element_interactions a11y_click_events_have_key_events -->
							<div
								class="row commit-row"
								class:selected={selected?.commit.hash === lc.commit.hash}
								style="position: absolute; top: {vRow.start}px; left: 0; right: 0; height: {ROW_HEIGHT}px;"
								role="button"
								tabindex="0"
								onclick={() => selectCommit(lc)}
								oncontextmenu={(e) => {
									e.preventDefault();
									ctxMenu = { x: e.clientX, y: e.clientY, commit: lc };
								}}
								onkeydown={(e) =>
									(e.key === "Enter" || e.key === " ") &&
									(e.preventDefault(), selectCommit(lc))}
							>
								<div class="col-branch">
									{#each visibleLabels(lc) as label}
										{@const pillColor = branchColorMap.get(label.name) ??
											getLaneColor(lc.color_index)}
										{@const pillTitle = !label.isTag && !label.isRemote && !label.isHead
											? `${label.name} — Double-click to checkout`
											: label.name}
										<span
											class="pill"
											class:pill-head={label.isHead}
											class:pill-checkoutable={!label.isTag && !label.isRemote && !label.isHead}
											style="color: {pillColor}; background: color-mix(in srgb, {pillColor} 18%, var(--bg-tertiary)); border-color: color-mix(in srgb, {pillColor} 35%, var(--bg-tertiary));"
											title={pillTitle}
											onclick={(e) => handleBranchPillClick(e, lc, label)}
										>
											{#if label.isHead}✓ {/if}{label.name}
										</span>
									{/each}
									{#if overflowCount(lc) > 0}
										<span
											class="pill-more"
											title={hiddenLabels(lc).map((l) => l.name).join('\n')}
										>
											+{overflowCount(lc)}
										</span>
									{/if}
									{#each tagByHash.get(lc.commit.hash) ?? [] as tag (tag.name)}
										<span
											class="pill pill-tag"
											style="color: var(--accent-yellow); background: color-mix(in srgb, var(--accent-yellow) 12%, var(--bg-tertiary)); border-color: color-mix(in srgb, var(--accent-yellow) 30%, var(--bg-tertiary));"
											title={tag.name}
										>
											🏷 {tag.name}
										</span>
									{/each}
								</div>
								<div class="col-graph"></div>
								<div class="labels-and-message">
									<span
										class="commit-hash"
										style="color: {getLaneColor(lc.color_index)}"
									>
										{lc.commit.short_hash}
									</span>
									<span class="message" title={lc.commit.message}>
										{lc.commit.message.split("\n")[0]}
									</span>
								</div>
								<span class="meta">
									{lc.commit.author_name} · {formatRelativeTime(lc.commit.timestamp)}
								</span>
							</div>
						{/if}
					{/each}
				</div>
			</div>
			<canvas
				bind:this={canvasEl}
				class="graph-canvas"
				aria-hidden="true"
			></canvas>
		</div>
	{/if}

	{#if tooltipCommit}
		<div
			class="commit-tooltip"
			style="left: {tooltipPos.x + 12}px; top: {tooltipPos.y + 12}px;"
			role="tooltip"
		>
			<div class="tooltip-hash">{tooltipCommit.commit.hash}</div>
			<div class="tooltip-message">{tooltipCommit.commit.message}</div>
			<div class="tooltip-author">
				{tooltipCommit.commit.author_name} &lt;{tooltipCommit.commit
					.author_email}&gt;
			</div>
			<div class="tooltip-date">
				{formatTooltipDate(tooltipCommit.commit.timestamp)}
			</div>
		</div>
	{/if}

	{#if ctxMenu}
		<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
		<div class="ctx-backdrop" onclick={() => (ctxMenu = null)} role="presentation"></div>

		<div class="ctx-menu" style="top: {ctxMenu.y}px; left: {ctxMenu.x}px">
			<div class="ctx-header">
				{ctxMenu.commit.commit.short_hash}
				<span class="ctx-msg">
					{ctxMenu.commit.commit.message.split("\n")[0].slice(0, 40)}
				</span>
			</div>

			<div class="ctx-divider"></div>

			<button class="ctx-item" onclick={() => handleCherryPick(ctxMenu!.commit)}>
				🍒 Cherry-pick onto current branch
			</button>

			<button class="ctx-item" onclick={() => handleRevert(ctxMenu!.commit)}>
				↩ Revert commit
			</button>

			<div class="ctx-divider"></div>

			<button class="ctx-item" onclick={() => handleReset(ctxMenu!.commit, "soft")}>
				↺ Reset to here (soft)
				<span class="ctx-hint">keep changes staged</span>
			</button>

			<button class="ctx-item" onclick={() => handleReset(ctxMenu!.commit, "mixed")}>
				↺ Reset to here (mixed)
				<span class="ctx-hint">keep changes unstaged</span>
			</button>

			<button class="ctx-item ctx-danger" onclick={() => handleReset(ctxMenu!.commit, "hard")}>
				↺ Reset to here (hard)
				<span class="ctx-hint">discard all changes</span>
			</button>

			<div class="ctx-divider"></div>

			<button class="ctx-item" onclick={() => handleCreateBranchHere(ctxMenu!.commit)}>
				⎇ Create branch from here
			</button>

			<button class="ctx-item" onclick={() => handleRebaseOnto(ctxMenu!.commit)}>
				⎇ Rebase current branch onto here
			</button>

			<div class="ctx-divider"></div>

			{#if !$bisectState?.active && !bisectBadPending}
				<button class="ctx-item" onclick={() => handleBisectBad(ctxMenu!.commit)}>
					🔍 Bisect: mark as bad
				</button>
			{:else if bisectBadPending}
				<button class="ctx-item" onclick={() => handleBisectGood(ctxMenu!.commit)}>
					🔍 Bisect: mark as good (start)
				</button>
			{/if}

			<button class="ctx-item" onclick={() => handleCopyHash(ctxMenu!.commit)}>
				📋 Copy commit hash
			</button>

			<button class="ctx-item" onclick={() => handleCopyMessage(ctxMenu!.commit)}>
				📋 Copy commit message
			</button>
		</div>
	{/if}
</div>

<style>
	.graph-root {
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 100%;
		overflow: hidden;
		min-height: 0;
	}

	.graph-header {
		display: flex;
		align-items: center;
		gap: 12px;
		color: var(--text-primary);
		padding: 8px 12px;
		font-size: 12px;
		background: var(--bg-secondary);
		flex-shrink: 0;
	}

	.header-count {
		flex-shrink: 0;
	}

	.search-box {
		display: flex;
		align-items: center;
		flex: 1;
		min-width: 0;
		max-width: 240px;
		position: relative;
	}

	.search-input {
		width: 100%;
		padding: 3px 24px 3px 8px;
		font-size: 12px;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 4px;
		color: var(--text-primary);
	}
	.search-input::placeholder {
		color: var(--text-muted);
	}
	.search-input:focus {
		outline: none;
		border-color: var(--accent-blue);
	}

	.search-clear {
		position: absolute;
		right: 4px;
		top: 50%;
		transform: translateY(-50%);
		width: 20px;
		height: 20px;
		padding: 0;
		font-size: 16px;
		line-height: 1;
		background: transparent;
		border: none;
		color: var(--text-muted);
		cursor: pointer;
		border-radius: 4px;
	}
	.search-clear:hover {
		background: var(--border);
		color: var(--text-primary);
	}

	.filter-btn {
		width: 24px;
		height: 24px;
		border: none;
		background: transparent;
		color: var(--text-muted);
		cursor: pointer;
		border-radius: 4px;
		font-size: 12px;
		position: relative;
		flex-shrink: 0;
		margin-left: 4px;
	}
	.filter-btn:hover, .filter-btn.active {
		background: var(--bg-tertiary);
		color: var(--text-primary);
	}
	.filter-badge {
		position: absolute;
		top: 2px;
		right: 2px;
		width: 6px;
		height: 6px;
		background: var(--accent-blue);
		border-radius: 50%;
	}
	.filter-panel {
		padding: 8px 12px;
		background: var(--bg-tertiary);
		border-bottom: 1px solid var(--border);
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.filter-row {
		display: flex;
		gap: 8px;
	}
	.filter-label {
		flex: 1;
		font-size: 10px;
		color: var(--text-muted);
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	.filter-input {
		width: 100%;
		padding: 4px 6px;
		font-size: 11px;
		background: var(--bg-primary);
		border: 1px solid var(--border);
		border-radius: 3px;
		color: var(--text-primary);
	}
	.filter-input:focus {
		outline: none;
		border-color: var(--accent-blue);
	}
	.filter-actions {
		display: flex;
		gap: 6px;
		justify-content: flex-end;
	}
	.filter-apply {
		padding: 4px 12px;
		font-size: 11px;
		background: var(--accent-blue);
		color: white;
		border: none;
		border-radius: 3px;
		cursor: pointer;
	}
	.filter-apply:disabled {
		opacity: 0.5;
	}
	.bisect-bar {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 12px;
		background: rgba(88, 166, 255, 0.08);
		border-bottom: 1px solid rgba(88, 166, 255, 0.2);
		flex-shrink: 0;
		font-size: 11px;
	}
	.bisect-icon { font-size: 14px; }
	.bisect-text { font-weight: 600; color: var(--accent-blue); }
	.bisect-steps { color: var(--text-muted); }
	.bisect-found { color: var(--accent-green); font-weight: 600; }
	.bisect-actions { margin-left: auto; display: flex; gap: 4px; }
	.bisect-btn {
		padding: 2px 10px;
		font-size: 10px;
		border: 1px solid var(--border);
		border-radius: 3px;
		cursor: pointer;
		background: var(--bg-secondary);
		color: var(--text-secondary);
	}
	.bisect-btn:hover { background: var(--bg-tertiary); }
	.bisect-btn.good { color: var(--accent-green); border-color: var(--accent-green); }
	.bisect-btn.bad { color: var(--accent-red); border-color: var(--accent-red); }
	.bisect-btn.skip { color: var(--accent-orange); border-color: var(--accent-orange); }
	.bisect-btn.reset { color: var(--text-muted); }

	.filter-clear {
		padding: 4px 12px;
		font-size: 11px;
		background: transparent;
		color: var(--text-secondary);
		border: 1px solid var(--border);
		border-radius: 3px;
		cursor: pointer;
	}

	.graph-state {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-height: 120px;
		background: var(--bg-secondary);
	}

	.graph-loading {
		color: var(--text-muted);
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 12px;
	}

	.graph-spinner {
		width: 40px;
		height: 40px;
		border: 3px solid var(--border);
		border-top-color: var(--accent-blue);
		border-radius: 50%;
		animation: graph-spin 0.8s linear infinite;
	}

	.graph-loading-text {
		font-size: 13px;
		color: var(--text-muted);
	}
	@keyframes graph-spin {
		to {
			transform: rotate(360deg);
		}
	}

	.graph-empty {
		color: var(--text-muted);
		font-size: 13px;
	}

	.empty-icon {
		font-size: 48px;
		opacity: 0.5;
		margin-bottom: 8px;
	}

	.empty-text {
		font-size: 14px;
	}

	.graph-headers {
		display: flex;
		align-items: center;
		padding: 0px 12px;
		padding-bottom: 6px;
		font-size: 10px;
		font-weight: 600;
		color: var(--text-muted);
		text-transform: uppercase;
		letter-spacing: 0.5px;
		background: var(--bg-secondary);
		border-bottom: 1px solid var(--border);
		flex-shrink: 0;
	}

	.gh-branch {
		width: var(--branch-col-width);
		min-width: var(--branch-col-width);
		flex-shrink: 0;
	}
	.gh-graph {
		width: var(--graph-col-width);
		min-width: var(--graph-col-width);
		flex-shrink: 0;
	}
	.gh-message {
		flex: 1;
		min-width: 0;
	}

	.graph-scroll-wrap {
		position: relative;
		flex: 1;
		min-height: 0;
		height: 0;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.graph-scroll {
		flex: 1;
		overflow-y: auto;
		overflow-x: hidden;
		position: relative;
		min-height: 0;
		height: 100%;
		max-height: 100%;
		scroll-behavior: auto;
		transform: translateZ(0);
		-webkit-overflow-scrolling: touch;
	}

	.graph-scroll::-webkit-scrollbar {
		width: 6px;
	}
	.graph-scroll::-webkit-scrollbar-thumb {
		background: var(--border);
		border-radius: 3px;
	}

	.graph-canvas {
		position: absolute;
		left: var(--branch-col-width);
		top: 0;
		width: var(--graph-col-width);
		height: 100%;
		pointer-events: auto;
		z-index: 2;
	}

	.scroll-content {
		position: relative;
		width: 100%;
		min-width: min-content;
	}

	.row {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 0 12px;
		height: 28px;
		cursor: pointer;
		box-sizing: border-box;
		border-radius: 4px;
		contain: layout style;
	}

	.row:hover {
		will-change: background-color;
		background: var(--bg-tertiary);
	}

	.row.selected {
		background: var(--border);
	}

	.col-branch {
		width: var(--branch-col-width);
		flex-shrink: 0;
		padding: 0 6px 0 8px;
		display: flex;
		align-items: center;
		gap: 4px;
		overflow: hidden;
		min-width: 0;
		position: relative;
	}

	.pill {
		display: inline-flex;
		align-items: center;
		padding: 1px 7px;
		height: 17px;
		border-radius: 3px;
		font-size: 10px;
		font-weight: 500;
		white-space: nowrap;
		overflow: visible;
		text-overflow: clip;
		max-width: none !important;
		min-width: 0;
		flex-shrink: 0;
		border: 1px solid;
		letter-spacing: 0.1px;
		line-height: 1;
	}
	.pill-checkoutable {
		cursor: pointer;
	}
	.pill-checkoutable:hover {
		opacity: 0.9;
	}

	.col-graph {
		width: var(--graph-col-width);
		min-width: var(--graph-col-width);
		flex-shrink: 0;
		height: 100%;
		position: relative;
		overflow: visible;
		z-index: 1;
	}

	.labels-and-message {
		flex: 1;
		min-width: 0;
		display: flex;
		align-items: center;
		gap: 8px;
		overflow: hidden;
	}

	.commit-hash {
		font-family: "JetBrains Mono", monospace;
		font-size: 11px;
		flex-shrink: 0;
		margin-right: 8px;
		letter-spacing: 0.3px;
	}

	.pill-more {
		font-size: 10px;
		color: var(--text-muted);
		flex-shrink: 0;
		white-space: nowrap;
	}

	.pill-tag {
		font-size: 9px;
		gap: 2px;
	}

	.message {
		font-size: 13px;
		color: var(--text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.meta {
		font-size: 12px;
		color: var(--text-secondary);
		flex-shrink: 0;
	}

	.wip-label {
		font-family: "JetBrains Mono", monospace;
		font-size: 13px;
		color: var(--text-primary);
		font-weight: 600;
		margin-right: 8px;
	}

	.wip-count-badge {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-width: 20px;
		height: 18px;
		padding: 0 6px;
		border-radius: 10px;
		font-size: 11px;
		font-weight: 600;
		margin-left: 8px;
		background: color-mix(in srgb, var(--accent-green) 15%, transparent);
		color: var(--accent-green);
		border: 1px solid color-mix(in srgb, var(--accent-green) 30%, transparent);
	}

	.col-message .wip-clean {
		font-size: 11px;
		color: var(--text-muted);
		font-style: italic;
		margin-left: 8px;
	}

	.wip-clean {
		opacity: 0.5;
	}

	.commit-tooltip {
		position: fixed;
		z-index: 1000;
		pointer-events: none;
		background: var(--bg-tertiary);
		border: 1px solid var(--border);
		border-radius: 6px;
		padding: 12px;
		max-width: 400px;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
		font-size: 12px;
		line-height: 1.5;
	}

	.tooltip-hash {
		font-family: ui-monospace, monospace;
		color: var(--accent-blue);
		margin-bottom: 4px;
		word-break: break-all;
	}

	.tooltip-message {
		color: var(--text-primary);
		white-space: pre-wrap;
		word-break: break-word;
		margin-bottom: 8px;
	}

	.tooltip-author {
		color: var(--text-secondary);
		margin-bottom: 4px;
	}

	.tooltip-date {
		color: var(--text-muted);
	}

	.ctx-backdrop {
		position: fixed;
		inset: 0;
		z-index: 998;
	}

	.ctx-menu {
		position: fixed;
		z-index: 999;
		background: var(--bg-secondary);
		border: 1px solid var(--border);
		border-radius: 8px;
		padding: 4px;
		min-width: 240px;
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
		font-size: 13px;
	}

	.ctx-header {
		padding: 6px 10px 8px;
		font-size: 11px;
		color: var(--text-muted);
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.ctx-header .ctx-msg {
		color: var(--text-secondary);
		font-size: 12px;
	}

	.ctx-divider {
		height: 1px;
		background: var(--border);
		margin: 2px 0;
	}

	.ctx-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: 7px 10px;
		border: none;
		background: transparent;
		color: var(--text-primary);
		cursor: pointer;
		border-radius: 4px;
		font-size: 13px;
		text-align: left;
		gap: 8px;
	}

	.ctx-item:hover {
		background: var(--bg-tertiary);
	}

	.ctx-item.ctx-danger {
		color: var(--accent-red);
	}

	.ctx-item.ctx-danger:hover {
		background: rgba(248, 81, 73, 0.1);
	}

	.ctx-hint {
		font-size: 10px;
		color: var(--text-muted);
		margin-left: auto;
	}
</style>

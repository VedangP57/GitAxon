<script lang="ts">
	import { Application, Graphics, Container } from "pixi.js";
	import {
		
		Virtualizer,
		observeElementRect,
		observeElementOffset,
		elementScroll,
	} from "@tanstack/virtual-core";
	import {
		commits,
		branches,
		status,
		selectedCommit,
		selectCommit,
		loadMoreCommits,
		isLoading,
		rightPanelMode,
		showWip,
	} from "$lib/store";
	import { onDestroy, untrack } from "svelte";
	import type { BranchInfo, LanedCommit } from "$lib/types";

	const MAX_CANVAS_PX = 16000;
	const GRAPH_COL_WIDTH = 240;

	const ROW_HEIGHT = 36;
	const LANE_SPACING = 24;
	const LANE_OFFSET = 16;
	const DOT_RADIUS = 4;
	const LINE_WIDTH = 1.5;
	const CANVAS_WIDTH = 240;

	function buildLaneColors(): number[] {
		const cssVars = [
			"--accent-blue",
			"--accent-green",
			"--accent-purple",
			"--accent-orange",
			"--accent-red",
			"--accent-yellow",
			"--accent-teal",
			"--accent-pink",
		];
		const fallbacks = [
			0x58a6ff, 0x3fb950, 0xbc8cff, 0xf0883e, 0xf85149, 0xd29922,
			0x39d353, 0xff7b72,
		];
		return cssVars.map((v, i) => {
			const raw = getComputedStyle(document.documentElement)
				.getPropertyValue(v)
				.trim();
			if (!raw) return fallbacks[i];
			if (raw.startsWith("#")) {
				const n = parseInt(raw.slice(1), 16);
				return isNaN(n) ? fallbacks[i] : n;
			}
			return fallbacks[i];
		});
	}
	const LANE_COLORS_CSS = [
		"#8b949e",
		"#3fb950",
		"#d29922",
		"#bc8cff",
		"#f85149",
		"#39d353",
		"#ff7b72",
		"#adbac7",
	];

	function getLaneX(lane: number): number {
		return Math.min(LANE_OFFSET + lane * LANE_SPACING, CANVAS_WIDTH - 10);
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

	let containerRef = $state<HTMLDivElement | null>(null);
	let canvasContainer = $state<HTMLDivElement | null>(null);
	let app: Application | null = null;
	let canvasHostRef: HTMLElement | null = null; // container we appended the Pixi canvas to
	let isInitializing = false;
	let isInitialized = $state(false);
	let pixiFailed = $state(false);
	let fallbackCanvas = $state<HTMLCanvasElement | null>(null);
	let virtualizer = $state<Virtualizer<
		HTMLDivElement,
		HTMLDivElement
	> | null>(null);
	let virtualizerVersion = $state(0);
	let tooltipCommit = $state<LanedCommit | null>(null);

	let tooltipPos = $state({ x: 0, y: 0 });
	let searchQuery = $state("");
	let lastDrawnRef: LanedCommit[] = [];
	let scrollThrottled = false;

	const totalChanges = $derived($status.length);

	// StatusEntry: staged (bool), status ("M"|"A"|"D"|"WM"|"WD"|"?")
	const unstagedCount = $derived(
		$status.filter((f) => !f.staged).length,
	);
	const stagedCount = $derived(
		$status.filter((f) => f.staged).length,
	);

	const commitList = $derived.by(() => {
		const all = $commits;
		const q = searchQuery.trim().toLowerCase();
		if (!q) return all;
		return all.filter(
			(lc) =>
				lc.commit.message.toLowerCase().includes(q) ||
				lc.commit.author_name.toLowerCase().includes(q) ||
				lc.commit.hash.toLowerCase().includes(q) ||
				lc.commit.short_hash.toLowerCase().includes(q),
		);
	});
	const branchList = $derived($branches);
	const selected = $derived($selectedCommit);
	const totalHeight = $derived(
		Math.max(1, commitList.length + 1) * ROW_HEIGHT,
	);

	// Memoized branch lookup: hash -> BranchInfo[]
	const branchByHash = $derived.by(() => {
		const map = new Map<string, BranchInfo[]>();
		for (const b of branchList) {
			const existing = map.get(b.tipHash) ?? [];
			existing.push(b);
			map.set(b.tipHash, existing);
		}
		return map;
	});

	// Pre-compute lane state for every row — battle-tested algorithm
	const laneStates = $derived.by(() => {
		const commits = commitList;
		const states: Map<number, number>[] = [];
		const active = new Map<number, number>(); // lane -> colorIndex

		for (let i = 0; i < commits.length; i++) {
			const lc = commits[i];

			// Ensure current commit lane is active with its color
			active.set(lc.lane, lc.color_index);

			// Snapshot current state for this row
			states.push(new Map(active));

			// Update active lanes based on edges
			for (const edge of lc.edges) {
				if (edge.edge_type === "Straight") {
					active.set(edge.to_lane, edge.color_index);
				} else if (edge.edge_type === "Fork") {
					active.set(edge.to_lane, edge.color_index);
				} else if (edge.edge_type === "Merge") {
					// Merged branch ends here
					active.delete(edge.to_lane);
					// Main lane continues
					active.set(edge.from_lane, lc.color_index);
				}
			}

			// Remove lanes with no outgoing edges (branch tips)
			if (lc.edges.length === 0) {
				active.delete(lc.lane);
			}
		}
		return states;
	});

	// Lane remapping: compress gaps so [0, 3, 7] -> [0, 1, 2] for compact visual
	const laneRemap = $derived.by(() => {
		const states = laneStates;
		const allLanes = new Set<number>();
		for (const s of states) {
			for (const [lane] of s) allLanes.add(lane);
		}
		// Also include lanes from commits (for dots)
		for (const lc of commitList) {
			allLanes.add(lc.lane);
		}
		const sorted = [...allLanes].sort((a, b) => a - b);
		const remap = new Map<number, number>();
		sorted.forEach((lane, idx) => remap.set(lane, idx));
		return remap;
	});

	$effect(() => {
		const el = containerRef;
		const list = commitList;
		if (!el) return;

		let v = virtualizer;
		if (!v) {
			v = new Virtualizer({
				count: list.length,
				getScrollElement: () => containerRef,
				estimateSize: () => ROW_HEIGHT,
				scrollToFn: (offset, opts, instance) => {
					elementScroll(offset, opts, instance);
				},
				observeElementRect,
				observeElementOffset,
				overscan: 10,
				onChange: () => {
					// Defer to break effect loop: onChange -> virtualizerVersion++ -> re-render -> ResizeObserver -> measure -> onChange
					queueMicrotask(() => {
						virtualizerVersion++;
					});
				},
			});
			virtualizer = v;
			v._willUpdate();
		} else {
			v.setOptions({
				count: list.length,
				getScrollElement: () => containerRef,
				estimateSize: () => ROW_HEIGHT,
				scrollToFn: (offset, opts, instance) => {
					elementScroll(offset, opts, instance);
				},
				observeElementRect,
				observeElementOffset,
				overscan: 10,
				onChange: () => {
					// Defer to break effect loop: onChange -> virtualizerVersion++ -> re-render -> ResizeObserver -> measure -> onChange
					queueMicrotask(() => {
						virtualizerVersion++;
					});
				},
			});
			v._willUpdate();
		}
	});

	$effect(() => {
		const v = virtualizer;
		const el = containerRef;
		if (!v || !el) return;
		// Defer measurement until layout is ready (next frame)
		const id = requestAnimationFrame(() => {
			if (el.clientHeight > 0) {
				v.measure();
			}
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

	function ensureCanvasInContainer(container: HTMLElement) {
		if (!app) return;
		const canvas = app.canvas;
		// Re-attach if container was recreated (e.g. after loadRepo tore down DOM)
		const needsReattach =
			canvasHostRef !== container ||
			!document.contains(canvas);
		if (needsReattach) {
			canvasHostRef = container;
			container.appendChild(canvas);
			queueMicrotask(() => onScroll());
		}
	}

	async function initPixi(container: HTMLElement) {
		// Already initialized: ensure canvas is in the current container (handles DOM recreation)
		if (isInitialized && app) {
			ensureCanvasInContainer(container);
			return;
		}
		if (isInitializing) return;
		isInitializing = true;

		try {
			const pixiApp = new Application();
			const list = commitList;
			const totalHeight = list.length * ROW_HEIGHT;
			const canvasHeight = Math.min(
				Math.max(600, totalHeight),
				MAX_CANVAS_PX,
			);

			await pixiApp.init({
				width: GRAPH_COL_WIDTH,
				height: canvasHeight,
				backgroundAlpha: 0,
				antialias: true,
				preference: "webgl",
				resolution: window.devicePixelRatio || 1,
				autoDensity: true,
			});

			container.appendChild(pixiApp.canvas);
			canvasHostRef = container;
			pixiApp.canvas.style.position = "absolute";
			pixiApp.canvas.style.top = "0";
			pixiApp.canvas.style.left = "0";

			pixiApp.ticker.maxFPS = 60;
			pixiApp.ticker.start();

			app = pixiApp;
			isInitialized = true;
			isInitializing = false;

			console.log("[Pixi] Init complete");

			if (list.length > 0) {
				drawGraph(list);
			}
		} catch (err) {
			console.warn("[Pixi] WebGL failed, trying canvas:", err);
			isInitializing = true;
			try {
				const pixiApp = new Application();
				const list = commitList;
				const totalHeight = list.length * ROW_HEIGHT;
				const canvasHeight = Math.min(
					Math.max(600, totalHeight),
					MAX_CANVAS_PX,
				);

				await pixiApp.init({
					width: GRAPH_COL_WIDTH,
					height: canvasHeight,
					backgroundAlpha: 0,
					preference: "canvas",
					resolution: 1,
				});

				container.appendChild(pixiApp.canvas);
				canvasHostRef = container;
				pixiApp.canvas.style.position = "absolute";
				pixiApp.canvas.style.top = "0";
				pixiApp.canvas.style.left = "0";

				pixiApp.ticker.maxFPS = 60;
				pixiApp.ticker.start();

				app = pixiApp;
				isInitialized = true;
				if (list.length > 0) {
					drawGraph(list);
				}
				console.log("[Pixi] Init complete (canvas fallback)");
			} catch (err2) {
				console.error("[Pixi] Canvas also failed:", err2);
				pixiFailed = true;
			} finally {
				isInitializing = false;
			}
		}
	}

	// Reactive Pixi init: run when container and commits are both available
	$effect(() => {
		const container = canvasContainer;
		const list = commitList;
		if (!container || list.length === 0) return;
		initPixi(container);
	});

	// Draw ONCE when commits change or when Pixi becomes ready. NEVER on scroll.
	// Read isInitialized first so effect re-runs when Pixi finishes init
	$effect(() => {
		const ready = isInitialized;
		const pixiApp = app;
		if (!ready || !pixiApp) return;

		const current = commitList;
		if (current === lastDrawnRef) return;
		lastDrawnRef = current;

		untrack(() => {
			const newHeight = Math.min(
				Math.max(600, current.length * ROW_HEIGHT),
				MAX_CANVAS_PX,
			);
			if (pixiApp.renderer.height !== newHeight) {
				pixiApp.renderer.resize(GRAPH_COL_WIDTH, newHeight);
			}
			drawGraph(current);
		});
	});

	function drawFallbackGraph(canvas: HTMLCanvasElement, list: LanedCommit[]) {
		const ctx = canvas.getContext("2d");
		if (!ctx) return;

		const colors = buildLaneColors();
		const toHex = (n: number) =>
			"#" + (n & 0xffffff).toString(16).padStart(6, "0");

		const h = (list.length + 1) * ROW_HEIGHT;
		canvas.width = GRAPH_COL_WIDTH;
		canvas.height = h;
		ctx.clearRect(0, 0, canvas.width, canvas.height);

		// WIP row
		const wipY = ROW_HEIGHT / 2;
		const wipX = LANE_OFFSET;
		const radius = 6;
		const dashCount = 8;
		for (let i = 0; i < dashCount; i++) {
			const startAngle = (i / dashCount) * Math.PI * 2;
			const endAngle = ((i + 0.6) / dashCount) * Math.PI * 2;
			ctx.beginPath();
			ctx.arc(wipX, wipY, radius, startAngle, endAngle);
			ctx.strokeStyle = "#e6edf3";
			ctx.lineWidth = 1.5;
			ctx.stroke();
		}
		ctx.beginPath();
		ctx.arc(wipX, wipY, 2, 0, Math.PI * 2);
		ctx.fillStyle = "#e6edf3";
		ctx.fill();

		ctx.beginPath();
		ctx.moveTo(LANE_OFFSET, wipY);
		ctx.lineTo(LANE_OFFSET, ROW_HEIGHT);
		ctx.strokeStyle = toHex(colors[0]);
		ctx.lineWidth = LINE_WIDTH;
		ctx.stroke();

		// Lane states
		const laneStates: Map<number, number>[] = [];
		const active = new Map<number, number>();
		for (let i = 0; i < list.length; i++) {
			const lc = list[i];
			active.set(lc.lane, lc.color_index);
			laneStates.push(new Map(active));
			for (const edge of lc.edges) {
				if (
					edge.edge_type === "Straight" ||
					edge.edge_type === "Fork"
				) {
					active.set(edge.to_lane, edge.color_index);
				} else if (edge.edge_type === "Merge") {
					active.delete(edge.to_lane);
					active.set(edge.from_lane, lc.color_index);
				}
			}
			if (lc.edges.length === 0) active.delete(lc.lane);
		}

		// Vertical lines by color
		const pathsByColor = new Map<number, Path2D>();
		for (let i = 0; i < list.length; i++) {
			const rowIndex = i + 1;
			for (const [lane, colorIdx] of laneStates[i]) {
				const x = LANE_OFFSET + lane * LANE_SPACING;
				if (!pathsByColor.has(colorIdx)) {
					pathsByColor.set(colorIdx, new Path2D());
				}
				const p = pathsByColor.get(colorIdx)!;
				p.moveTo(x, rowIndex * ROW_HEIGHT);
				p.lineTo(x, (rowIndex + 1) * ROW_HEIGHT);
			}
		}

		// Bezier curves
		for (let i = 0; i < list.length; i++) {
			const rowIndex = i + 1;
			for (const edge of list[i].edges) {
				if (edge.edge_type !== "Straight") {
					const fromX =
						LANE_OFFSET + edge.from_lane * LANE_SPACING;
					const toX = LANE_OFFSET + edge.to_lane * LANE_SPACING;
					const fromY =
						rowIndex * ROW_HEIGHT + ROW_HEIGHT / 2;
					const toY =
						(rowIndex + 1) * ROW_HEIGHT + ROW_HEIGHT / 2;
					if (!pathsByColor.has(edge.color_index)) {
						pathsByColor.set(edge.color_index, new Path2D());
					}
					const p = pathsByColor.get(edge.color_index)!;
					p.moveTo(fromX, fromY);
					p.bezierCurveTo(
						fromX,
						fromY + ROW_HEIGHT * 0.6,
						toX,
						toY - ROW_HEIGHT * 0.6,
						toX,
						toY,
					);
				}
			}
		}

		for (const [colorIdx, p] of pathsByColor) {
			ctx.strokeStyle = toHex(colors[colorIdx % colors.length]);
			ctx.lineWidth = LINE_WIDTH;
			ctx.stroke(p);
		}

		// Dots
		for (let i = 0; i < list.length; i++) {
			const lc = list[i];
			const rowIndex = i + 1;
			const x = LANE_OFFSET + lc.lane * LANE_SPACING;
			const y = rowIndex * ROW_HEIGHT + ROW_HEIGHT / 2;
			const color = toHex(colors[lc.color_index % colors.length]);

			ctx.beginPath();
			ctx.arc(x, y, DOT_RADIUS + 1.5, 0, Math.PI * 2);
			ctx.fillStyle = "#ffffff";
			ctx.fill();

			ctx.beginPath();
			ctx.arc(x, y, DOT_RADIUS, 0, Math.PI * 2);
			ctx.fillStyle = color;
			ctx.fill();
		}
	}

	function getLineG(
		linesByColor: Map<number, Graphics>,
		linesContainer: Container,
		colorIdx: number,
	): Graphics {
		const idx = colorIdx % 8;
		if (!linesByColor.has(idx)) {
			const g = new Graphics();
			linesByColor.set(idx, g);
			linesContainer.addChild(g);
		}
		return linesByColor.get(idx)!;
	}

	function drawGraph(list: LanedCommit[]) {
		if (!app) return;

		const colors = buildLaneColors();
		console.log(
			"[Pixi] Lane colors:",
			colors.map((c) => "#" + c.toString(16)),
		);

		app.ticker.stop();

		app.stage.removeChildren();

		const linesContainer = new Container();
		const dotsContainer = new Container();
		app.stage.addChild(linesContainer);
		app.stage.addChild(dotsContainer);

		const linesByColor = new Map<number, Graphics>();

		const hasWip = true;

		if (hasWip) {
			const wipY = ROW_HEIGHT / 2;
			const wipX = LANE_OFFSET;

			const wipDot = new Graphics();
			const radius = 6;
			const dashCount = 8;

			for (let i = 0; i < dashCount; i++) {
				const startAngle = (i / dashCount) * Math.PI * 2;
				const endAngle = ((i + 0.6) / dashCount) * Math.PI * 2;
				wipDot.arc(wipX, wipY, radius, startAngle, endAngle);
				wipDot.stroke({ width: 1.5, color: 0xe6edf3 });
			}

			wipDot.circle(wipX, wipY, 2);
			wipDot.fill({ color: 0xe6edf3 });

			dotsContainer.addChild(wipDot);

			const lineG = getLineG(linesByColor, linesContainer, 0);
			lineG.moveTo(LANE_OFFSET, wipY);
			lineG.lineTo(LANE_OFFSET, ROW_HEIGHT);
		}

		// Compute lane states
		const laneStates: Map<number, number>[] = [];
		const active = new Map<number, number>();

		for (let i = 0; i < list.length; i++) {
			const lc = list[i];
			active.set(lc.lane, lc.color_index);
			laneStates.push(new Map(active));

			for (const edge of lc.edges) {
				if (
					edge.edge_type === "Straight" ||
					edge.edge_type === "Fork"
				) {
					active.set(edge.to_lane, edge.color_index);
				} else if (edge.edge_type === "Merge") {
					active.delete(edge.to_lane);
					active.set(edge.from_lane, lc.color_index);
				}
			}
			if (lc.edges.length === 0) {
				active.delete(lc.lane);
			}
		}

		// Draw vertical lane lines — single Graphics per color (max 8)
		for (let i = 0; i < list.length; i++) {
			const rowIndex = i + 1;
			for (const [lane, colorIdx] of laneStates[i]) {
				const x = LANE_OFFSET + lane * LANE_SPACING;
				const g = getLineG(linesByColor, linesContainer, colorIdx);
				g.moveTo(x, rowIndex * ROW_HEIGHT);
				g.lineTo(x, (rowIndex + 1) * ROW_HEIGHT);
			}
		}

		// Draw bezier curves — same color Graphics
		for (let i = 0; i < list.length; i++) {
			const rowIndex = i + 1;
			for (const edge of list[i].edges) {
				if (edge.edge_type !== "Straight") {
					const fromX = LANE_OFFSET + edge.from_lane * LANE_SPACING;
					const toX = LANE_OFFSET + edge.to_lane * LANE_SPACING;
					const fromY = rowIndex * ROW_HEIGHT + ROW_HEIGHT / 2;
					const toY = (rowIndex + 1) * ROW_HEIGHT + ROW_HEIGHT / 2;

					const g = getLineG(
						linesByColor,
						linesContainer,
						edge.color_index,
					);
					g.moveTo(fromX, fromY);
					g.bezierCurveTo(
						fromX,
						fromY + ROW_HEIGHT * 0.6,
						toX,
						toY - ROW_HEIGHT * 0.6,
						toX,
						toY,
					);
				}
			}
		}

		// Stroke all line Graphics at once
		for (const [colorIdx, g] of linesByColor) {
			g.stroke({
				width: LINE_WIDTH,
				color: colors[colorIdx % colors.length],
			});
		}

		// Draw dots on top
		let maxLane = 0;
		for (let i = 0; i < list.length; i++) {
			const lc = list[i];
			const rowIndex = i + 1;
			if (lc.lane > maxLane) maxLane = lc.lane;
			const x = LANE_OFFSET + lc.lane * LANE_SPACING;
			const y = rowIndex * ROW_HEIGHT + ROW_HEIGHT / 2;
			const color = colors[lc.color_index % colors.length];

			const dot = new Graphics();
			dot.circle(x, y, DOT_RADIUS + 1.5);
			dot.fill({ color: 0xffffff });
			dot.circle(x, y, DOT_RADIUS);
			dot.fill({ color });

			dot.eventMode = "static";
			dot.cursor = "pointer";
			dot.on("pointerover", () => dot.scale.set(1.3));
			dot.on("pointerout", () => dot.scale.set(1.0));
			dot.on("pointertap", () => selectCommit(lc));

			dotsContainer.addChild(dot);
		}

		const totalHeight = (list.length + 1) * ROW_HEIGHT;
		const canvasHeight = Math.min(totalHeight, MAX_CANVAS_PX);
		app.renderer.resize(GRAPH_COL_WIDTH, canvasHeight);
		app.renderer.render(app.stage);
		app.ticker.start();

		// Sync scroll position so graph lines up with list on first paint
		queueMicrotask(() => onScroll());

		console.log(
			`[GitFast] Graph: ${list.length} commits, ${maxLane + 1} lanes`,
		);
	}

	// Scroll: GPU stage.y transform only. Zero CPU. No render(), no drawGraph().
	function onScroll() {
		if (!app || !containerRef) return;
		const totalH = Math.max(1, commitList.length + 1) * ROW_HEIGHT;
		const ratio =
			totalH > MAX_CANVAS_PX ? MAX_CANVAS_PX / totalH : 1;
		app.stage.y = -containerRef.scrollTop * ratio;
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

	$effect(() => {
		const el = containerRef;
		const ready = isInitialized; // reactive dep: run when Pixi is ready
		if (!el || !ready || !app) return;
		el.addEventListener("scroll", onScroll, { passive: true });
		return () => el.removeEventListener("scroll", onScroll);
	});

	// Fallback canvas: draw when Pixi failed and we have commits
	$effect(() => {
		if (!pixiFailed) return;
		const canvas = fallbackCanvas;
		const list = commitList;
		if (!canvas || list.length === 0) return;
		drawFallbackGraph(canvas, list);
	});

	onDestroy(() => {
		if (app) {
			app.ticker.stop();
			app.destroy(true, { children: true, texture: true });
			app = null;
		}
		canvasHostRef = null;
		isInitialized = false;
		isInitializing = false;
	});

	const virtualItems = $derived.by(() => {
		virtualizerVersion;
		const items = virtualizer?.getVirtualItems() ?? [];
		// Fallback: when virtualizer returns empty but we have commits, render all
		if (items.length === 0 && commitList.length > 0) {
			return commitList.map((_, i) => ({
				key: i,
				index: i,
				start: i * ROW_HEIGHT,
				end: (i + 1) * ROW_HEIGHT,
				size: ROW_HEIGHT,
				lane: 0,
			}));
		}
		return items;
	});

	function handleWipClick() {
		showWip();
	}
</script>

<div class="graph-root">
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
					onclick={() => (searchQuery = "")}
					title="Clear search"
					type="button"
				>
					×
				</button>
			{/if}
		</div>
	</div>
	{#if $isLoading}
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
		<div class="scroll-container" bind:this={containerRef} role="list">
			<div class="scroll-content" style="height: {totalHeight}px;">
				{#if pixiFailed}
					<div
						class="canvas-col"
						style="height: {totalHeight}px; position: relative;"
					>
						<canvas
							bind:this={fallbackCanvas}
							width={240}
							height={totalHeight}
						></canvas>
					</div>
				{:else}
					<div
						class="canvas-col canvas-container"
						bind:this={canvasContainer}
						style="height: {totalHeight}px; position: relative;"
					></div>
				{/if}
				<div
					class="list-col"
					style="height: {totalHeight}px; position: relative;"
				>
					<!-- WIP row (always visible as first row) -->
					<!-- svelte-ignore a11y_no_noninteractive_element_interactions a11y_click_events_have_key_events -->
					<div
						class="row wip-row"
						class:wip-clean={$status.length === 0}
						class:selected={$rightPanelMode === "wip" &&
							!$selectedCommit}
						style="position: absolute; top: 0; width: 100%; left: 0; right: 0; height: {ROW_HEIGHT}px;"
						role="button"
						tabindex="0"
						onclick={handleWipClick}
						onkeydown={(e) =>
							(e.key === "Enter" || e.key === " ") &&
							(e.preventDefault(), handleWipClick())}
					>
						<span class="hash wip-hash-placeholder"></span>
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

					{#each virtualItems as item (item.key)}
						{@const lc = commitList[item.index]}
						{@const branchLabels =
							branchByHash.get(lc.commit.hash) ?? []}
						<!-- svelte-ignore a11y_no_noninteractive_element_interactions a11y_click_events_have_key_events -->
						<div
							class="row"
							class:selected={selected?.commit.hash ===
								lc.commit.hash}
							style="position: absolute; top: {item.start + ROW_HEIGHT}px; width: 100%; left: 0; right: 0;"
							role="button"
							tabindex="0"
							onclick={() => selectCommit(lc)}
							onkeydown={(e) =>
								(e.key === "Enter" || e.key === " ") &&
								(e.preventDefault(), selectCommit(lc))}
							onmouseenter={(e) => {
								tooltipCommit = lc;
								tooltipPos = { x: e.clientX, y: e.clientY };
							}}
							onmousemove={(e) => {
								if (
									tooltipCommit?.commit.hash ===
									lc.commit.hash
								) {
									tooltipPos = { x: e.clientX, y: e.clientY };
								}
							}}
							onmouseleave={() => {
								tooltipCommit = null;
							}}
						>
							<span
								class="hash"
								style="color: {LANE_COLORS_CSS[
									lc.color_index % 8
								]}"
							>
								{lc.commit.short_hash}
							</span>
							<div class="labels-and-message">
								{#each branchLabels as b}
									<span
										class="branch-pill"
										class:remote={b.isRemote}
										class:head={b.isHead}
									>
										{b.name}
									</span>
								{/each}
								<span class="message" title={lc.commit.message}>
									{lc.commit.message
										.split("\n")[0]
										.slice(0, 60)}
									{lc.commit.message.split("\n")[0].length >
									60
										? "…"
										: ""}
								</span>
							</div>
							<span class="meta">
								{lc.commit.author_name} · {formatRelativeTime(
									lc.commit.timestamp,
								)}
							</span>
						</div>
					{/each}
				</div>
			</div>
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
		padding: 4px 24px 4px 8px;
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

	.scroll-container {
		flex: 1;
		overflow-y: auto;
		overflow-x: auto;
		position: relative;
		min-height: 0;
		scroll-behavior: auto;
		transform: translateZ(0);
		-webkit-overflow-scrolling: touch;
	}

	.scroll-content {
		display: flex;
		flex-direction: row;
		flex-wrap: nowrap;
		width: 100%;
		min-width: min-content;
		position: relative;
	}

	.canvas-col {
		width: 240px;
		min-width: 240px;
		flex-shrink: 0;
		position: relative;
	}

	.canvas-container {
		/* In flow (no position: absolute) so graph doesn't overlay list - both clickable */
		width: 240px;
		min-width: 240px;
		height: 100%;
		flex-shrink: 0;
		position: relative;
		pointer-events: auto;
		z-index: 1;
	}

	.canvas-container :global(canvas) {
		position: absolute;
		top: 0;
		left: 0;
		pointer-events: auto;
	}

	.canvas-col :global(canvas) {
		position: absolute;
		top: 0;
		left: 0;
		display: block;
		width: 240px !important;
		min-width: 240px;
		height: 100% !important;
	}

	.list-col {
		flex: 1;
		min-width: 0;
		background: var(--bg-secondary);
		color: var(--text-primary);
	}

	.row {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 0 12px;
		height: 36px;
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

	.hash {
		font-family: ui-monospace, monospace;
		font-size: 12px;
		min-width: 56px;
		flex-shrink: 0;
	}

	.labels-and-message {
		flex: 1;
		min-width: 0;
		display: flex;
		align-items: center;
		gap: 8px;
		overflow: hidden;
	}

	.branch-pill {
		font-size: 10px;
		padding: 2px 6px;
		border-radius: 4px;
		background: var(--accent-blue);
		color: var(--bg-primary);
		flex-shrink: 0;
	}

	.branch-pill.remote {
		background: var(--text-secondary);
		color: var(--bg-primary);
	}

	.branch-pill.head {
		background: var(--text-primary);
		color: var(--bg-primary);
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

	.wip-hash-placeholder {
		visibility: hidden;
	}

	.wip-label {
		font-family: "JetBrains Mono", monospace;
		font-size: 13px;
		color: #e6edf3;
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

	.wip-badge {
		background: rgba(248, 81, 73, 0.2);
		color: #f85149;
		border: 1px solid rgba(248, 81, 73, 0.4);
		border-radius: 10px;
		padding: 0 7px;
		font-size: 11px;
		font-weight: 600;
		margin-right: 4px;
	}

	.wip-staged-badge {
		background: rgba(63, 185, 80, 0.2);
		color: #3fb950;
		border: 1px solid rgba(63, 185, 80, 0.4);
		border-radius: 10px;
		padding: 0 7px;
		font-size: 11px;
		font-weight: 600;
	}

	.wip-clean {
		opacity: 0.5;
	}

	.wip-clean-label {
		color: #484f58;
		font-size: 12px;
		font-style: italic;
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
</style>

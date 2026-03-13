<script lang="ts">
	import { Application, Graphics, Container } from 'pixi.js';
	import {
		Virtualizer,
		observeElementRect,
		observeElementOffset,
		elementScroll
	} from '@tanstack/virtual-core';
	import {
		commits,
		branches,
		selectedCommit,
		selectCommit,
		loadMoreCommits,
		isLoading
	} from '$lib/store';
	import { onMount, onDestroy } from 'svelte';
	import type { BranchInfo, LanedCommit } from '$lib/types';

	const ROW_HEIGHT = 36;
	const LANE_SPACING = 24;
	const LANE_OFFSET = 16;
	const DOT_RADIUS = 4;
	const LINE_WIDTH = 1.5;
	const CANVAS_WIDTH = 240;

	const LANE_COLORS = [
		0x58a6ff, 0x3fb950, 0xd29922, 0xbc8cff,
		0xf85149, 0x39d353, 0xff7b72, 0x79c0ff
	];
	const LANE_COLORS_CSS = [
		'#58a6ff', '#3fb950', '#d29922', '#bc8cff',
		'#f85149', '#39d353', '#ff7b72', '#79c0ff'
	];

	function getLaneX(lane: number): number {
		return Math.min(LANE_OFFSET + lane * LANE_SPACING, CANVAS_WIDTH - 10);
	}

	function formatRelativeTime(timestamp: number): string {
		const sec = Math.floor((Date.now() - timestamp * 1000) / 1000);
		if (sec < 60) return 'just now';
		if (sec < 3600) return `${Math.floor(sec / 60)} min ago`;
		if (sec < 86400) return `${Math.floor(sec / 3600)} hours ago`;
		if (sec < 2592000) return `${Math.floor(sec / 86400)} days ago`;
		if (sec < 31536000) return `${Math.floor(sec / 2592000)} months ago`;
		return `${Math.floor(sec / 31536000)} years ago`;
	}

	function formatTooltipDate(timestamp: number): string {
		const d = new Date(timestamp * 1000);
		const date = d.toLocaleDateString(undefined, {
			month: 'long',
			day: 'numeric',
			year: 'numeric'
		});
		const time = d.toLocaleTimeString(undefined, {
			hour: 'numeric',
			minute: '2-digit',
			hour12: true
		});
		return `${date} at ${time}`;
	}

	let containerRef = $state<HTMLDivElement | null>(null);
	let canvasContainer = $state<HTMLDivElement | null>(null);
	let app = $state<Application | null>(null);
	let initialized = $state(false);
	let pixiFailed = $state(false);
	let fallbackCanvas = $state<HTMLCanvasElement | null>(null);
	let virtualizer = $state<Virtualizer<HTMLDivElement, HTMLDivElement> | null>(null);
	let virtualizerVersion = $state(0);
	let tooltipCommit = $state<LanedCommit | null>(null);

	let tooltipPos = $state({ x: 0, y: 0 });
	let searchQuery = $state('');

	const commitList = $derived.by(() => {
		const all = $commits;
		const q = searchQuery.trim().toLowerCase();
		if (!q) return all;
		return all.filter(
			(lc) =>
				lc.commit.message.toLowerCase().includes(q) ||
				lc.commit.author_name.toLowerCase().includes(q) ||
				lc.commit.hash.toLowerCase().includes(q) ||
				lc.commit.short_hash.toLowerCase().includes(q)
		);
	});
	const branchList = $derived($branches);
	const selected = $derived($selectedCommit);
	const totalHeight = $derived(Math.max(1, commitList.length) * ROW_HEIGHT);

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
				if (edge.edge_type === 'Straight') {
					active.set(edge.to_lane, edge.color_index);
				} else if (edge.edge_type === 'Fork') {
					active.set(edge.to_lane, edge.color_index);
				} else if (edge.edge_type === 'Merge') {
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
				}
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
				}
			});
			v._willUpdate();
		}
	});

	$effect(() => {
		const v = virtualizer;
		const el = containerRef;
		if (v && el?.clientHeight) {
			v.measure();
		}
	});

	$effect(() => {
		const el = containerRef;
		if (!el) return;
		const ro = new ResizeObserver(() => virtualizer?.measure());
		ro.observe(el);
		return () => ro.disconnect();
	});

	// Pixi init: use $effect to run when container is ready (conditional rendering)
	$effect(() => {
		const container = canvasContainer;
		const list = commitList;
		if (!container || list.length === 0 || initialized) return;

		(async () => {
			console.log('Pixi: starting init');
			try {
				const pixiApp = new Application();
				await pixiApp.init({
					width: 240,
					height: Math.max(600, list.length * ROW_HEIGHT),
					background: 0x0d1117,
					antialias: true,
					resolution: window.devicePixelRatio || 1,
					autoDensity: true
				});

				// Append canvas to container
				container.appendChild(pixiApp.canvas);
				console.log('Pixi: init complete, canvas appended');

				// Style the canvas
				pixiApp.canvas.style.position = 'absolute';
				pixiApp.canvas.style.top = '0';
				pixiApp.canvas.style.left = '0';

				app = pixiApp;
				initialized = true;

				// Draw initial graph
				if (list.length > 0) {
					drawGraph();
				}
			} catch (err) {
				console.error('Pixi init failed:', err);
				pixiFailed = true;
			}
		})();
	});

	function drawGraph() {
		if (!app) return;
		const list = commitList;

		console.log('Pixi: drawing', list.length, 'commits');

		// Clear stage
		app.stage.removeChildren();

		const linesContainer = new Container();
		const dotsContainer = new Container();
		app.stage.addChild(linesContainer);
		app.stage.addChild(dotsContainer);

		// Compute lane states
		const laneStates: Map<number, number>[] = [];
		const active = new Map<number, number>();

		for (let i = 0; i < list.length; i++) {
			const lc = list[i];
			active.set(lc.lane, lc.color_index);
			laneStates.push(new Map(active));

			for (const edge of lc.edges) {
				if (edge.edge_type === 'Straight' || edge.edge_type === 'Fork') {
					active.set(edge.to_lane, edge.color_index);
				} else if (edge.edge_type === 'Merge') {
					active.delete(edge.to_lane);
					active.set(edge.from_lane, lc.color_index);
				}
			}
			if (lc.edges.length === 0) {
				active.delete(lc.lane);
			}
		}

		// Draw vertical lane lines
		for (let i = 0; i < list.length; i++) {
			for (const [lane, colorIdx] of laneStates[i]) {
				const x = LANE_OFFSET + lane * LANE_SPACING;
				const y1 = i * ROW_HEIGHT;
				const y2 = (i + 1) * ROW_HEIGHT;

				const g = new Graphics();
				g.moveTo(x, y1);
				g.lineTo(x, y2);
				g.stroke({ width: LINE_WIDTH, color: LANE_COLORS[colorIdx % 8] });
				linesContainer.addChild(g);
			}

			// Draw bezier curves
			for (const edge of list[i].edges) {
				if (edge.edge_type !== 'Straight') {
					const fromX = LANE_OFFSET + edge.from_lane * LANE_SPACING;
					const toX = LANE_OFFSET + edge.to_lane * LANE_SPACING;
					const fromY = i * ROW_HEIGHT + ROW_HEIGHT / 2;
					const toY = (i + 1) * ROW_HEIGHT + ROW_HEIGHT / 2;

					const g = new Graphics();
					g.moveTo(fromX, fromY);
					g.bezierCurveTo(
						fromX,
						fromY + ROW_HEIGHT * 0.6,
						toX,
						toY - ROW_HEIGHT * 0.6,
						toX,
						toY
					);
					g.stroke({
						width: LINE_WIDTH,
						color: LANE_COLORS[edge.color_index % 8]
					});
					linesContainer.addChild(g);
				}
			}
		}

		// Draw dots on top
		for (let i = 0; i < list.length; i++) {
			const lc = list[i];
			const x = LANE_OFFSET + lc.lane * LANE_SPACING;
			const y = i * ROW_HEIGHT + ROW_HEIGHT / 2;
			const color = LANE_COLORS[lc.color_index % 8];

			const dot = new Graphics();
			dot.circle(x, y, DOT_RADIUS + 1.5);
			dot.fill({ color: 0xffffff });
			dot.circle(x, y, DOT_RADIUS);
			dot.fill({ color });

			dot.eventMode = 'static';
			dot.cursor = 'pointer';
			dot.on('pointerover', () => dot.scale.set(1.3));
			dot.on('pointerout', () => dot.scale.set(1.0));
			dot.on('pointertap', () => selectCommit(lc));

			dotsContainer.addChild(dot);
		}

		app.renderer.render(app.stage);
		console.log('Pixi: draw complete');
	}

	// Reactive redraw when commits change (after init)
	$effect(() => {
		const commitCount = commitList.length;

		if (!initialized || !app || commitCount === 0) return;

		// Resize renderer to fit all commits
		const newHeight = commitCount * ROW_HEIGHT;
		app.renderer.resize(240, newHeight);

		drawGraph();
	});

	// Scroll sync: move stage when list scrolls
	function onScroll() {
		if (app && containerRef) {
			app.stage.y = -containerRef.scrollTop;
			app.renderer.render(app.stage);
		}
	}

	$effect(() => {
		const el = containerRef;
		if (!el || !app) return;
		el.addEventListener('scroll', onScroll, { passive: true });
		return () => el.removeEventListener('scroll', onScroll);
	});

	onDestroy(() => {
		if (app) {
			app.destroy(true, { children: true, texture: true });
			app = null;
		}
		initialized = false;
	});

	function handleScroll() {
		const el = containerRef;
		if (!el) return;
		const { scrollTop, scrollHeight, clientHeight } = el;
		const distFromBottom = scrollHeight - scrollTop - clientHeight;
		if (distFromBottom < ROW_HEIGHT * 20) {
			loadMoreCommits();
		}
	}

	$effect(() => {
		const el = containerRef;
		if (!el) return;
		el.addEventListener('scroll', handleScroll, { passive: true });
		return () => el.removeEventListener('scroll', handleScroll);
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
				lane: 0
			}));
		}
		return items;
	});
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
					onclick={() => (searchQuery = '')}
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
		<div
			class="scroll-container"
			bind:this={containerRef}
			role="list"
		>
			<div class="scroll-content" style="height: {totalHeight}px;">
				{#if pixiFailed}
					<div class="canvas-col" style="height: {totalHeight}px; position: relative;">
						<canvas bind:this={fallbackCanvas} width={240} height={totalHeight}></canvas>
					</div>
				{:else}
					<div
						class="canvas-col"
						bind:this={canvasContainer}
						style="height: {totalHeight}px; position: relative;"
					></div>
				{/if}
				<div class="list-col" style="height: {totalHeight}px; position: relative;">
					{#each virtualItems as item (item.key)}
					{@const lc = commitList[item.index]}
					{@const branchLabels = branchByHash.get(lc.commit.hash) ?? []}
					<!-- svelte-ignore a11y_no_noninteractive_element_interactions a11y_click_events_have_key_events -->
					<div
						class="row"
						class:selected={selected?.commit.hash === lc.commit.hash}
						style="position: absolute; top: {item.start}px; width: 100%; left: 0; right: 0;"
						role="button"
						tabindex="0"
						onclick={() => selectCommit(lc)}
						onkeydown={(e) =>
							(e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), selectCommit(lc))}
						onmouseenter={(e) => {
							tooltipCommit = lc;
							tooltipPos = { x: e.clientX, y: e.clientY };
						}}
						onmousemove={(e) => {
							if (tooltipCommit?.commit.hash === lc.commit.hash) {
								tooltipPos = { x: e.clientX, y: e.clientY };
							}
						}}
						onmouseleave={() => {
							tooltipCommit = null;
						}}
					>
						<span
							class="hash"
							style="color: {LANE_COLORS_CSS[lc.color_index % 8]}"
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
								{lc.commit.message.split('\n')[0].slice(0, 60)}
								{lc.commit.message.split('\n')[0].length > 60 ? '…' : ''}
							</span>
						</div>
						<span class="meta">
							{lc.commit.author_name} · {formatRelativeTime(lc.commit.timestamp)}
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
				{tooltipCommit.commit.author_name} &lt;{tooltipCommit.commit.author_email}&gt;
			</div>
			<div class="tooltip-date">{formatTooltipDate(tooltipCommit.commit.timestamp)}</div>
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
		color: white;
		padding: 8px 12px;
		font-size: 12px;
		background: #161b22;
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
		background: var(--bg-primary);
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
	}

	.canvas-col {
		width: 240px;
		min-width: 240px;
		flex-shrink: 0;
		position: relative;
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
		background: var(--bg-primary);
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
		background: #161b22;
	}

	.row.selected {
		background: #21262d;
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
		background: #58a6ff;
		color: #0d1117;
		flex-shrink: 0;
	}

	.branch-pill.remote {
		background: #8b949e;
		color: #0d1117;
	}

	.branch-pill.head {
		background: #3fb950;
		color: #0d1117;
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

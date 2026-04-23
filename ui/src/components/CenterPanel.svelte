<script lang="ts">
	import CommitGraph from './CommitGraph.svelte';
	import DiffViewer from './DiffViewer.svelte';
	import FileHistoryPanel from './FileHistoryPanel.svelte';
	import ConflictResolver from './ConflictResolver.svelte';
	import RebasePanel from './RebasePanel.svelte';
	import PrReviewPanel from './PrReviewPanel.svelte';
	import { centerView } from '$lib/store';

	let { leftPanelOpen = true }: { leftPanelOpen?: boolean } = $props();
</script>

<div class="center-panel">
	<div class="view-layer" class:active={$centerView === 'graph'}>
		<CommitGraph {leftPanelOpen} />
	</div>
	<div class="view-layer" class:active={$centerView === 'diff'}>
		<DiffViewer />
	</div>
	<div class="view-layer" class:active={$centerView === 'file-history'}>
		<FileHistoryPanel />
	</div>
	<div class="view-layer" class:active={$centerView === 'conflict'}>
		<ConflictResolver />
	</div>
	<div class="view-layer" class:active={$centerView === 'rebase'}>
		<RebasePanel />
	</div>
	<div class="view-layer" class:active={$centerView === 'pr-review'}>
		<PrReviewPanel />
	</div>
</div>

<style>
	.center-panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
		position: relative;
		background: var(--bg-secondary);
	}

	.view-layer {
		position: absolute;
		inset: 0;
		visibility: hidden;
		pointer-events: none;
		z-index: 0;
	}

	.view-layer.active {
		visibility: visible;
		pointer-events: auto;
		z-index: 10;
		display: flex;
		flex-direction: column;
	}
</style>

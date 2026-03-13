<script lang="ts">
	import CommitGraph from './CommitGraph.svelte';
	import DiffViewer from './DiffViewer.svelte';
	import { centerView } from '$lib/store';
</script>

<div class="center-panel">
	{#if $centerView === 'diff'}
		<DiffViewer />
	{:else}
		<CommitGraph />
	{/if}
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
</style>

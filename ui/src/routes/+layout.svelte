<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import '../app.css';
	import favicon from '$lib/assets/favicon.svg';
	import Welcome from '$components/Welcome.svelte';
	import AppShell from '$components/AppShell.svelte';
	import { get } from 'svelte/store';
	import { currentRepo, loadRepo, clearSelection } from '$lib/store';

	let { children } = $props();

	function handleKeydown(e: KeyboardEvent) {
		const isMac = navigator.platform.toLowerCase().includes('mac');
		const mod = isMac ? e.metaKey : e.ctrlKey;

		// Ctrl/Cmd+R: refresh current repo
		if (mod && e.key === 'r') {
			e.preventDefault();
			const repo = get(currentRepo);
			if (repo) loadRepo(repo);
			return;
		}

		// Ctrl/Cmd+Enter: commit (if staged files and message)
		if (mod && e.key === 'Enter') {
			e.preventDefault();
			window.dispatchEvent(new CustomEvent('gitfast:commit'));
			return;
		}

		// Escape: clear selected commit and file
		if (e.key === 'Escape') {
			e.preventDefault();
			clearSelection();
		}
	}

	onMount(() => {
		window.addEventListener('keydown', handleKeydown);
	});

	onDestroy(() => {
		window.removeEventListener('keydown', handleKeydown);
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

{#if $currentRepo === null}
	<Welcome />
{:else}
	<AppShell />
{/if}

{@render children()}

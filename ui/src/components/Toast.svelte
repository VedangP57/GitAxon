<script lang="ts">
	import { toast } from '$lib/toast';
</script>

{#if $toast.length > 0}
	<div class="toast-stack" role="status" aria-live="polite">
		{#each $toast as t (t.id)}
			<button
				type="button"
				class="toast toast-{t.type}"
				onclick={() => toast.update((arr) => arr.filter((x) => x.id !== t.id))}
				onkeydown={(e) => e.key === 'Enter' && toast.update((arr) => arr.filter((x) => x.id !== t.id))}
			>
				{t.message}
			</button>
		{/each}
	</div>
{/if}

<style>
	.toast-stack {
		position: fixed;
		bottom: 20px;
		right: 20px;
		z-index: 9999;
		display: flex;
		flex-direction: column;
		gap: 8px;
		pointer-events: none;
	}

	.toast-stack > .toast {
		pointer-events: auto;
	}

	.toast {
		display: block;
		width: 100%;
		padding: 12px 20px;
		border-radius: 8px;
		font-size: 13px;
		font-weight: 500;
		background: var(--bg-tertiary);
		color: var(--text-primary);
		border: none;
		border-left: 4px solid;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
		cursor: pointer;
		animation: toast-in 0.2s ease-out;
		transition: opacity 0.2s ease, transform 0.2s ease;
		text-align: left;
		font-family: inherit;
	}

	.toast-success {
		border-left-color: var(--accent-green);
	}

	.toast-error {
		border-left-color: var(--accent-red);
	}

	.toast-info {
		border-left-color: var(--accent-blue);
	}

	@keyframes toast-in {
		from {
			opacity: 0;
			transform: translateX(24px);
		}
		to {
			opacity: 1;
			transform: translateX(0);
		}
	}
</style>

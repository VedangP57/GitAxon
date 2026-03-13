<script lang="ts">
	import { toast } from "$lib/toast";
	import { fade, fly } from "svelte/transition";
</script>

{#if $toast.length > 0}
	<div class="toast-stack" role="status" aria-live="polite">
		{#each $toast as t (t.id)}
			<div
				class="toast toast-{t.type}"
				in:fly={{ y: 20, duration: 300 }}
				out:fade={{ duration: 200 }}
			>
				<div class="toast-icon">
					{#if t.type === "success"}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							width="20"
							height="20"
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							stroke-width="2"
							stroke-linecap="round"
							stroke-linejoin="round"
						>
							<path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
							<polyline points="22 4 12 14.01 9 11.01" />
						</svg>
					{:else if t.type === "error"}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							width="20"
							height="20"
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							stroke-width="2"
							stroke-linecap="round"
							stroke-linejoin="round"
						>
							<circle cx="12" cy="12" r="10" />
							<line x1="15" y1="9" x2="9" y2="15" />
							<line x1="9" y1="9" x2="15" y2="15" />
						</svg>
					{:else}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							width="20"
							height="20"
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							stroke-width="2"
							stroke-linecap="round"
							stroke-linejoin="round"
						>
							<circle cx="12" cy="12" r="10" />
							<line x1="12" y1="16" x2="12" y2="12" />
							<line x1="12" y1="8" x2="12.01" y2="8" />
						</svg>
					{/if}
				</div>
				<span class="toast-message">{t.message}</span>
				<button
					class="toast-close"
					aria-label="Close"
					onclick={() =>
						toast.update((arr) => arr.filter((x) => x.id !== t.id))}
				>
					<svg
						xmlns="http://www.w3.org/2000/svg"
						width="16"
						height="16"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
					>
						<line x1="18" y1="6" x2="6" y2="18" />
						<line x1="6" y1="6" x2="18" y2="18" />
					</svg>
				</button>
			</div>
		{/each}
	</div>
{/if}

<style>
	.toast-stack {
		position: fixed;
		bottom: 24px;
		right: 24px;
		z-index: 9999;
		display: flex;
		flex-direction: column;
		gap: 12px;
		pointer-events: none;
	}

	.toast {
		pointer-events: auto;
		display: flex;
		align-items: center;
		gap: 12px;
		width: 320px;
		padding: 14px 16px;
		border-radius: 12px;
		background: rgba(22, 22, 22, 0.75);
		backdrop-filter: blur(12px);
		-webkit-backdrop-filter: blur(12px);
		border: 1px solid var(--border);
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
		transition: transform 0.2s ease, box-shadow 0.2s ease;
	}

	.toast:hover {
		transform: translateY(-2px);
		box-shadow: 0 12px 32px rgba(0, 0, 0, 0.5);
	}

	.toast-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}

	.toast-success .toast-icon {
		color: var(--accent-green);
	}

	.toast-error .toast-icon {
		color: var(--accent-red);
	}

	.toast-info .toast-icon {
		color: var(--accent-blue);
	}

	.toast-message {
		flex: 1;
		font-size: 13px;
		font-weight: 500;
		color: var(--text-primary);
		line-height: 1.4;
		text-align: left;
	}

	.toast-close {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 4px;
		background: transparent;
		border: none;
		color: var(--text-muted);
		cursor: pointer;
		border-radius: 6px;
		flex-shrink: 0;
		transition: color 0.2s, background 0.2s;
	}

	.toast-close:hover {
		color: var(--text-primary);
		background: rgba(255, 255, 255, 0.1);
	}
</style>

import { writable } from 'svelte/store';

export interface ToastMessage {
	id: number;
	message: string;
	type: 'success' | 'error' | 'info';
}

const toastStore = writable<ToastMessage[]>([]);
let nextId = 0;
const timeoutMap = new Map<number, ReturnType<typeof setTimeout>>();

export const toast = toastStore;

export function showToast(message: string, type: 'success' | 'error' | 'info' = 'success') {
	const id = ++nextId;
	toastStore.update((arr) => [...arr, { id, message, type }]);

	timeoutMap.set(
		id,
		setTimeout(() => {
			toastStore.update((arr) => arr.filter((t) => t.id !== id));
			timeoutMap.delete(id);
		}, 3000)
	);
}

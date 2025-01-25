// stores.ts
import { writable } from 'svelte/store';


export type Message = {
	text: string;
	type: 'success' | 'error' | 'warning' | 'info';
	id: symbol;
}

export const messageStore = writable<Message[]>([]);

export const addMessage = (
	text: string,
	type: 'success' | 'error' | 'warning' | 'info'
) => {
	const id = Symbol();
	messageStore.update(messages => [...messages, { text, type, id }]);

	setTimeout(() => {
		messageStore.update(messages => messages.filter(m => m.id !== id));
	}, 3000);
}

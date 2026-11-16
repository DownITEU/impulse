import { useSyncExternalStore } from 'react';

import type { Command, View } from '@/api';

import { initial, reduce, type State } from './state';

const RETRY_FIRST = 1000;
const RETRY_LAST = 10_000;
const NOTE_LIFE = 4000;

let state: State = initial;
let socket: WebSocket | null = null;
const listeners = new Set<() => void>();

function update(next: State): void {
    state = next;
    for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
    listeners.add(listener);
    return () => listeners.delete(listener);
}

export function useSession(): State {
    return useSyncExternalStore(subscribe, () => state);
}

/** A view from the app; the demo feeds them too. */
export function receive(view: View): void {
    update(reduce(state, view));
    if (view.type === 'note') {
        const note = state.notes[state.notes.length - 1];
        window.setTimeout(
            () => update({ ...state, notes: state.notes.filter((kept) => kept !== note) }),
            NOTE_LIFE,
        );
    }
}

/** A game's cover, served once the window has shown it. */
export const coverOf = (game: number): string => `/art/${game}`;

/** The link is up; the demo has none and says so too. */
export function opened(): void {
    update({ ...state, link: 'open', refused: false });
}

/** Dropped while the socket is down: the views that come back with it are the truth. */
export function send(command: Command): void {
    if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify(command));
}

/** Pairs with the code the address brought (the QR code's), out of the address bar first; `true` if taken. */
async function pair(): Promise<boolean> {
    const code = location.hash.slice(1);
    if (!code) return false;
    history.replaceState(null, '', location.pathname + location.search);
    const answer = await fetch('/pair', { method: 'POST', body: code }).catch(() => null);
    update({ ...state, refused: !answer?.ok });
    return answer?.ok ?? false;
}

/** Pairs, then keeps one socket open. */
export async function connect(): Promise<void> {
    await pair();
    open(RETRY_FIRST);
    // A code scanned into this tab changes only the fragment; its cookie needs a new socket.
    window.addEventListener('hashchange', async () => {
        if (!(await pair())) return;
        const old = socket;
        socket = null;
        old?.close();
        open(RETRY_FIRST);
    });
    // A phone drops sockets while asleep: try again as it wakes.
    document.addEventListener('visibilitychange', () => {
        if (document.visibilityState === 'visible' && !socket) open(RETRY_FIRST);
    });
}

function open(retry: number): void {
    const ws = new WebSocket(`ws://${location.host}/ws`);
    socket = ws;
    let wasOpen = false;
    ws.onopen = () => {
        wasOpen = true;
        opened();
    };
    ws.onmessage = (event) => receive(JSON.parse(event.data) as View);
    ws.onclose = () => {
        if (socket !== ws) return;
        socket = null;
        // The next socket sends every view again: until its run and values come, nothing acts.
        update({ ...state, link: wasOpen ? 'connecting' : 'lost', values: null, run: null });
        window.setTimeout(
            () => socket || open(wasOpen ? RETRY_FIRST : Math.min(retry * 2, RETRY_LAST)),
            retry,
        );
    };
}

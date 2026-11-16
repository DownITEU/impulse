import { type Game, type Sheet, VERSION, type View } from '@/api';

type Of<T extends View['type']> = Extract<View, { type: T }>;

/** A sheet whose cheats arrived. */
export type Ready = Extract<Sheet, { state: 'ready' }>;

export type Run = Of<'run'>;

export type Trainer = Of<'trainer'>;

/** `lost`: never opened, or the server is gone; retried either way. */
export type Link = 'connecting' | 'open' | 'lost';

export type Note = { id: number; text: string };

/** The newest of each view the app sent; the panel keeps nothing else. */
export type State = {
    link: Link;
    /** The code this page came with was refused: used, expired, or tried too often. */
    refused: boolean;
    hello: Of<'hello'> | null;
    games: Game[];
    trainer: Of<'trainer'> | null;
    values: Of<'values'> | null;
    run: Of<'run'> | null;
    notes: Note[];
};

/** Notes shown at once; older ones go first. */
const NOTES = 3;

export const initial: State = {
    link: 'connecting',
    refused: false,
    hello: null,
    games: [],
    trainer: null,
    values: null,
    run: null,
    notes: [],
};

let noted = 0;

export function reduce(state: State, view: View): State {
    switch (view.type) {
        case 'hello':
            return { ...state, hello: view };
        case 'library':
            return { ...state, games: view.games };
        case 'trainer':
            return { ...state, trainer: view };
        case 'values':
            return { ...state, values: view };
        case 'run':
            return { ...state, run: view };
        case 'note':
            noted += 1;
            return {
                ...state,
                notes: [...state.notes, { id: noted, text: view.text }].slice(-NOTES),
            };
    }
}

/** The shown trainer's values: none until a values view names its sheet. */
export function valuesOf(state: State): number[] | null {
    return state.values && state.values.sheet === state.trainer?.sheet ? state.values.values : null;
}

/** The shown trainer's game is live and the socket is open: its controls act. */
export function isLive(state: State): boolean {
    return (
        state.link === 'open' &&
        state.run?.state === 'live' &&
        state.run.game === state.trainer?.game
    );
}

/** The app speaks another protocol: this page is older than the app. */
export function isOutdated(state: State): boolean {
    return state.hello !== null && state.hello.version !== VERSION;
}

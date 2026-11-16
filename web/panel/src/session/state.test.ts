import { describe, expect, it } from 'vitest';

import { initial, isLive, type Run, reduce, type State, valuesOf } from './state';

const run: Run = { type: 'run', state: 'live', game: 0, since: 0, why: null };
const shown: State = {
    ...initial,
    link: 'open',
    trainer: { type: 'trainer', sheet: 2, game: 0, trainer: { state: 'pending' } },
    run,
};

describe('the session state', () => {
    it('shows values only for the trainer view they name', () => {
        expect(valuesOf(reduce(shown, { type: 'values', sheet: 1, values: [1] }))).toBeNull();
        expect(valuesOf(reduce(shown, { type: 'values', sheet: 2, values: [1] }))).toEqual([1]);
    });

    it('lets controls act only on the live game while the socket is open', () => {
        expect(isLive(shown)).toBe(true);
        expect(isLive({ ...shown, link: 'connecting' })).toBe(false);
        expect(isLive({ ...shown, run: { ...run, game: 1 } })).toBe(false);
        expect(isLive({ ...shown, run: { ...run, state: 'starting' } })).toBe(false);
    });

    it('keeps the last three notes', () => {
        const noted = ['a', 'b', 'c', 'd'].reduce(
            (state, text) => reduce(state, { type: 'note', text }),
            initial,
        );
        expect(noted.notes.map((note) => note.text)).toEqual(['b', 'c', 'd']);
    });
});

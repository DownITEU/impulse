import { cleanup, screen } from '@testing-library/preact';
import { afterEach, describe, expect, it } from 'vitest';

import type { Game } from '@/api';
import type { Run } from '@/session/state';
import { renderIn } from '@/shared/testing';

import { LibraryDrawer } from './LibraryDrawer';

const game = (handle: number, name: string, more: Partial<Game> = {}): Game => ({
    game: handle,
    name,
    favorite: false,
    open: false,
    live: false,
    startable: false,
    art: false,
    ...more,
});

const games = [
    game(0, 'ENSHROUDED', { live: true }),
    game(1, 'ICARUS', { favorite: true, startable: true }),
    game(2, 'RIMWORLD'),
];
const run: Run = { type: 'run', state: 'live', game: 0, since: 0, why: null };

afterEach(cleanup);

describe('the library drawer', () => {
    it('lists the run first, then favorites, then the rest', () => {
        renderIn('en', <LibraryDrawer games={games} run={run} online onClose={() => undefined} />);
        const names = screen
            .getAllByRole('heading', { level: 3 })
            .map((heading) => heading.textContent);
        expect(names).toEqual(['Now playing', 'Favorites', 'All games']);
        expect(screen.getAllByRole('button', { name: 'Stop the trainer' })).toHaveLength(1);
    });

    it('starts only a startable game while online', () => {
        const { rerender } = renderIn(
            'en',
            <LibraryDrawer games={games} run={null} online onClose={() => undefined} />,
        );
        const starts = () =>
            screen
                .getAllByRole('button', { name: 'Start the game and the trainer' })
                .map((button) => (button as HTMLButtonElement).disabled);
        expect(starts()).toEqual([false, true, true]);
        rerender(
            <LibraryDrawer games={games} run={null} online={false} onClose={() => undefined} />,
        );
        expect(starts()).toEqual([true, true, true]);
    });
});

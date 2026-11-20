import { cleanup, fireEvent, screen } from '@testing-library/preact';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { Game } from '@/api';
import { initial, type State } from '@/session/state';
import { renderIn } from '@/shared/testing';

import { blocked, SessionPlaceholder } from './SessionPlaceholder';

const hello: State['hello'] = { type: 'hello', version: 1, language: 'fr', accent: '#b4f000' };
const game: Game = {
    game: 3,
    name: 'ICARUS',
    favorite: false,
    open: true,
    live: false,
    startable: true,
    art: false,
};
const trainer: State['trainer'] = {
    type: 'trainer',
    sheet: 1,
    game: 3,
    trainer: { state: 'pending' },
};

afterEach(cleanup);

describe('the session placeholders', () => {
    it('tell why no trainer shows, the refusal first', () => {
        expect(blocked({ ...initial, refused: true, hello }, game)).toBe('refused');
        expect(blocked({ ...initial, hello: { ...hello, version: 2 } }, game)).toBe('outdated');
        expect(blocked({ ...initial, link: 'lost' }, null)).toBe('offline');
        expect(blocked(initial, null)).toBe('connecting');
        expect(blocked({ ...initial, hello }, null)).toBe('pick');
        expect(blocked({ ...initial, hello, trainer }, game)).toBeNull();
    });

    it('speak French and open the library', () => {
        const openLibrary = vi.fn();
        renderIn('fr', <SessionPlaceholder why="pick" onOpenLibrary={openLibrary} />);
        fireEvent.click(screen.getByRole('button', { name: 'Ouvrir la bibliothèque' }));
        expect(screen.getByText('Choisissez un jeu')).toBeTruthy();
        expect(openLibrary).toHaveBeenCalledOnce();
    });

    it('fall back to English for a language the panel does not ship', () => {
        renderIn('xx', <SessionPlaceholder why="offline" onOpenLibrary={() => undefined} />);
        expect(screen.getByText('Not connected')).toBeTruthy();
    });
});

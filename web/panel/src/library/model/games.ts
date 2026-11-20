import type { Game } from '@/api';
import type { Run } from '@/session/state';
import { spot } from '@/shared/lib/ui';

/** Favorites first, then the app's order, among the games whose name has `query`. */
export function arranged(games: Game[], query: string): { favorites: Game[]; rest: Game[] } {
    const shown = games.filter((game) => spot(game.name, query) !== null);
    return {
        favorites: shown.filter((game) => game.favorite),
        rest: shown.filter((game) => !game.favorite),
    };
}

export function initials(name: string): string {
    return name
        .split(/[\s:]+/)
        .filter(Boolean)
        .slice(0, 2)
        .map((word) => [...word][0])
        .join('')
        .toUpperCase();
}

/** The run is starting or live for `game`: the one a stop can end. */
export function isRunning(run: Run | null, game: Game): boolean {
    return run?.game === game.game && (run.state === 'starting' || run.state === 'live');
}

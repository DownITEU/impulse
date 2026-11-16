import { useState } from 'react';

import type { Game } from '@/api';
import { coverOf } from '@/session/store';

import { initials } from '../model/games';

type GameCoverProps = {
    game: Game;
    live: boolean;
    size?: 'sm' | 'lg';
};

const SIZE_CLASSES = {
    lg: 'size-16 rounded-[10px] text-[10px]',
    sm: 'size-11 rounded-[9px] text-[9px]',
};

/** Keyed by its game where the game can change: a failed cover stays failed. */
export const GameCover = ({ game, live, size = 'sm' }: GameCoverProps) => {
    // The app answers 404 until the window has shown the cover once: the initials stand in.
    const [failed, setFailed] = useState(false);

    return (
        <div
            className={`relative shrink-0 overflow-hidden border border-white/6 bg-[linear-gradient(135deg,color-mix(in_oklab,var(--deck-accent)_28%,transparent),rgba(0,0,0,0.48))] shadow-[inset_0_0_0_1px_rgba(255,255,255,.04),0_2px_6px_rgba(0,0,0,.35)] ${SIZE_CLASSES[size]}`}
        >
            {game.art && !failed ? (
                <img
                    alt=""
                    className="absolute inset-0 size-full object-cover"
                    src={coverOf(game.game)}
                    loading="lazy"
                    onError={() => setFailed(true)}
                />
            ) : (
                <div className="absolute inset-0 flex items-center justify-center font-mono font-bold tracking-wider text-white/85 drop-shadow">
                    {initials(game.name)}
                </div>
            )}
            <div className="absolute inset-0 bg-[linear-gradient(180deg,rgba(255,255,255,.14),transparent_35%,rgba(0,0,0,.42))]" />
            {live ? (
                <span className="absolute right-1 top-1 size-1.5 rounded-full bg-(--deck-accent) shadow-[0_0_6px_var(--deck-accent)]" />
            ) : null}
        </div>
    );
};

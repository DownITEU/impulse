import type { Game } from '@/api';
import { useLanguage, useT } from '@/i18n/texts';
import type { Run } from '@/session/state';
import { cn } from '@/shared/lib/ui';
import { Icon } from '@/shared/ui/Icon';

import { formatClock } from '../model/clock';
import { GameActions } from './GameActions';
import { GameCover } from './GameCover';
import { useElapsed } from './use-clock';

type GameHeaderProps = {
    game: Game;
    /** The run, whichever game it is for. */
    run: Run | null;
    online: boolean;
};

/** The open game over its trainer: its run, favorite, start and stop. */
export const GameHeader = ({ game, run, online }: GameHeaderProps) => {
    const ours = run?.game === game.game ? run : null;

    return (
        <section className="relative mb-3 mt-3.5 overflow-hidden rounded-[14px] border border-white/10 bg-[linear-gradient(180deg,rgba(255,255,255,0.04),rgba(255,255,255,0.015))] p-3.5 shadow-[0_8px_32px_-12px_rgba(0,0,0,.5),inset_0_1px_0_rgba(255,255,255,.06)]">
            <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_90%_-20%,color-mix(in_oklab,var(--deck-accent)_28%,transparent),transparent_55%)]" />
            <div className="relative flex items-center gap-3">
                <GameCover key={game.game} game={game} live={ours?.state === 'live'} size="lg" />
                <div className="min-w-0 flex-1">
                    <RunLine run={ours} />
                    <h2 className="mt-1 truncate text-base font-bold leading-tight text-(--deck-fg)">
                        {game.name}
                    </h2>
                </div>
                <GameActions game={game} run={run} online={online} size="md" />
            </div>
            {ours?.state === 'failed' && ours.why ? (
                <div className="relative mt-3 flex gap-2 rounded-[8px] border border-red-400/25 bg-red-500/10 px-2.5 py-2 text-[11.5px] leading-5 text-red-100">
                    <Icon className="mt-0.5 size-3.5 shrink-0" name="alert" />
                    <span>{ours.why}</span>
                </div>
            ) : null}
        </section>
    );
};

/** The run of this game: its state and, live, its clock. */
const RunLine = ({ run }: { run: Run | null }) => {
    const t = useT();
    const tag = useLanguage();
    const live = run?.state === 'live';
    const elapsed = useElapsed(run?.since ?? null, live);
    const caption = () => {
        switch (run?.state) {
            case 'live':
                return t('run-live');
            case 'starting':
                return t('run-starting');
            case 'stopping':
                return t('run-stopping');
            case 'failed':
                return t('run-failed');
            default:
                return t('run-idle');
        }
    };

    return (
        <div
            className={cn(
                'flex items-center gap-1.5 font-mono text-[9.5px] font-bold uppercase tracking-[0.16em]',
                live
                    ? 'text-(--deck-accent)'
                    : run?.state === 'failed'
                      ? 'text-red-300'
                      : 'text-(--deck-fg-4)',
            )}
        >
            {live ? (
                <span className="size-1.5 rounded-full bg-(--deck-accent) shadow-[0_0_6px_var(--deck-accent)] motion-safe:animate-[breathe_2s_ease-in-out_infinite]" />
            ) : null}
            <span>{caption()}</span>
            {live && elapsed !== null ? (
                <span className="tabular-nums">{formatClock(tag, elapsed)}</span>
            ) : null}
        </div>
    );
};

import { type ReactNode, useState } from 'react';

import type { Game } from '@/api';
import { useT } from '@/i18n/texts';
import type { Run } from '@/session/state';
import { send } from '@/session/store';
import { cn, spot } from '@/shared/lib/ui';
import { Icon, type IconName } from '@/shared/ui/Icon';
import { IconButton } from '@/shared/ui/IconButton';
import { SearchInput } from '@/shared/ui/SearchInput';

import { arranged, isRunning } from '../model/games';
import { GameActions } from './GameActions';
import { GameCover } from './GameCover';

type LibraryDrawerProps = {
    games: Game[];
    run: Run | null;
    online: boolean;
    onClose: () => void;
};

export const LibraryDrawer = ({ games, run, online, onClose }: LibraryDrawerProps) => {
    const t = useT();
    const [query, setQuery] = useState('');
    const running = games.find((game) => isRunning(run, game));
    const { favorites, rest } = arranged(
        games.filter((game) => game !== running),
        query,
    );
    const row = (game: Game) => (
        <GameRow
            key={game.game}
            game={game}
            run={run}
            online={online}
            query={query}
            onClose={onClose}
        />
    );

    return (
        <div className="flex h-full flex-col">
            <header className="remote-glass-header flex items-center gap-2.5 border-b px-3.5 py-3.5">
                <div className="min-w-0 flex-1">
                    <h2 className="text-lg font-bold text-(--deck-fg)">{t('library-title')}</h2>
                    <p className="mt-0.5 font-mono text-[11px] text-(--deck-fg-4)">
                        {t('library-count', { count: games.length })}
                    </p>
                </div>
                <IconButton label={t('library-close')} icon="x" onClick={onClose} />
            </header>
            <div className="border-b border-white/6 px-3.5 py-2.5">
                <SearchInput value={query} placeholder={t('library-search')} onChange={setQuery} />
            </div>
            <div className="remote-scrollbar-hidden min-h-0 flex-1 overflow-y-auto overscroll-contain pb-6">
                {running ? (
                    <GameSection accent count={1} icon="dot" title={t('library-playing')}>
                        {row(running)}
                    </GameSection>
                ) : null}
                {favorites.length > 0 ? (
                    <GameSection
                        count={favorites.length}
                        icon="star-filled"
                        title={t('library-favorites')}
                    >
                        {favorites.map(row)}
                    </GameSection>
                ) : null}
                {rest.length > 0 ? (
                    <GameSection count={rest.length} title={t('library-all')}>
                        {rest.map(row)}
                    </GameSection>
                ) : null}
                {!running && favorites.length + rest.length === 0 ? (
                    <p className="px-8 py-10 text-center text-[13px] text-(--deck-fg-4)">
                        {query ? t('library-no-match', { query }) : t('library-empty')}
                    </p>
                ) : null}
            </div>
        </div>
    );
};

type GameSectionProps = {
    title: string;
    count: number;
    icon?: IconName;
    accent?: boolean;
    children: ReactNode;
};

const GameSection = ({ title, count, icon, accent = false, children }: GameSectionProps) => {
    const tone = accent ? 'text-(--deck-accent)' : 'text-(--deck-fg-4)';
    return (
        <section className="mt-2">
            <div className="flex items-center gap-2 px-3.5 pb-1.5 pt-3.5">
                {icon ? <Icon className={cn('size-3', tone)} name={icon} stroke={2} /> : null}
                <h3
                    className={cn(
                        'font-mono text-[10px] font-bold uppercase tracking-[0.18em]',
                        tone,
                    )}
                >
                    {title}
                </h3>
                <div className="h-px flex-1 bg-white/6" />
                <span className="font-mono text-[10px] text-(--deck-fg-4)">{count}</span>
            </div>
            {children}
        </section>
    );
};

type GameRowProps = {
    game: Game;
    run: Run | null;
    online: boolean;
    query: string;
    onClose: () => void;
};

const GameRow = ({ game, run, online, query, onClose }: GameRowProps) => {
    const t = useT();
    const running = isRunning(run, game);
    const show = () => {
        send({ type: 'open', game: game.game });
        onClose();
    };

    return (
        <article
            className={cn(
                'mx-2 mb-1 flex items-center gap-2.5 rounded-[10px] border px-2.5 py-2.5 shadow-[inset_0_1px_0_rgba(255,255,255,0.04)]',
                running
                    ? 'border-[color-mix(in_oklab,var(--deck-accent)_24%,transparent)] bg-[color-mix(in_oklab,var(--deck-accent)_7%,transparent)]'
                    : 'border-white/7 bg-white/2.5',
            )}
        >
            <button
                type="button"
                disabled={!online}
                className="flex min-w-0 flex-1 items-center gap-2.5 text-left disabled:cursor-not-allowed"
                onClick={show}
            >
                <GameCover game={game} live={game.live} />
                <span className="min-w-0 flex-1">
                    <span className="block truncate text-[13.5px] font-semibold text-(--deck-fg)">
                        {highlight(game.name, query)}
                    </span>
                    {game.open ? (
                        <span className="mt-0.5 block font-mono text-[10.5px] tracking-[0.02em] text-(--deck-accent)">
                            {t('library-open')}
                        </span>
                    ) : null}
                </span>
            </button>
            <GameActions game={game} run={run} online={online} size="sm" />
        </article>
    );
};

function highlight(name: string, query: string): ReactNode {
    const found = spot(name, query);
    if (!found || found[0] === found[1]) return name;
    const [start, end] = found;
    return (
        <>
            {name.slice(0, start)}
            <span className="rounded-[3px] bg-[color-mix(in_oklab,var(--deck-accent)_25%,transparent)] px-0.5 text-(--deck-fg)">
                {name.slice(start, end)}
            </span>
            {name.slice(end)}
        </>
    );
}

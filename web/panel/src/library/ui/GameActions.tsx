import type { Game } from '@/api';
import { useT } from '@/i18n/texts';
import type { Run } from '@/session/state';
import { send } from '@/session/store';
import { IconButton } from '@/shared/ui/IconButton';

import { isRunning } from '../model/games';

type GameActionsProps = {
    game: Game;
    run: Run | null;
    online: boolean;
    size: 'sm' | 'md';
};

/** Favorite, and start or stop: the same in the library and over the trainer. */
export const GameActions = ({ game, run, online, size }: GameActionsProps) => {
    const t = useT();

    return (
        <div className="flex shrink-0 gap-1">
            <IconButton
                size={size}
                active={game.favorite}
                label={game.favorite ? t('game-unfavorite') : t('game-favorite')}
                icon={game.favorite ? 'star-filled' : 'star'}
                onClick={() => send({ type: 'favorite', game: game.game })}
            />
            {isRunning(run, game) ? (
                <IconButton
                    size={size}
                    danger
                    disabled={!online}
                    label={t('game-stop')}
                    icon="stop"
                    onClick={() => send({ type: 'stop', game: game.game })}
                />
            ) : (
                <IconButton
                    size={size}
                    accent
                    disabled={!online || !game.startable}
                    label={t('game-start')}
                    icon="play"
                    onClick={() => send({ type: 'start', game: game.game })}
                />
            )}
        </div>
    );
};

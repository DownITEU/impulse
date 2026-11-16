import type { Game } from '@/api';
import { useT } from '@/i18n/texts';
import { isOutdated, type State } from '@/session/state';
import { PlaceholderState } from '@/shared/ui/PlaceholderState';

export type Blocked = 'refused' | 'outdated' | 'offline' | 'connecting' | 'pick';

/** Why no trainer shows, `null` when it can: the open game is listed and the app speaks our protocol. */
export function blocked(state: State, game: Game | null): Blocked | null {
    if (state.refused) return 'refused';
    if (isOutdated(state)) return 'outdated';
    if (!state.hello) return state.link === 'lost' ? 'offline' : 'connecting';
    return game && state.trainer ? null : 'pick';
}

type SessionPlaceholderProps = {
    why: Blocked;
    onOpenLibrary: () => void;
};

export const SessionPlaceholder = ({ why, onOpenLibrary }: SessionPlaceholderProps) => {
    const t = useT();
    switch (why) {
        case 'refused':
            return (
                <PlaceholderState
                    icon="plug"
                    title={t('placeholder-refused')}
                    sub={t('placeholder-refused-body')}
                />
            );
        case 'outdated':
            return (
                <PlaceholderState
                    icon="alert"
                    title={t('placeholder-outdated')}
                    sub={t('placeholder-outdated-body')}
                    action={{ label: t('placeholder-reload'), run: () => location.reload() }}
                />
            );
        case 'offline':
            return (
                <PlaceholderState
                    icon="plug"
                    title={t('placeholder-offline')}
                    sub={t('placeholder-offline-body')}
                />
            );
        case 'connecting':
            return <PlaceholderState icon="loader" title={t('placeholder-connecting')} />;
        case 'pick':
            return (
                <PlaceholderState
                    icon="gamepad-variant-outline"
                    title={t('placeholder-pick')}
                    sub={t('placeholder-pick-body')}
                    action={{ label: t('placeholder-browse'), run: onOpenLibrary }}
                />
            );
    }
};

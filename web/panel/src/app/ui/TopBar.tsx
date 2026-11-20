import { useT } from '@/i18n/texts';
import type { Link } from '@/session/state';
import { IconButton } from '@/shared/ui/IconButton';

import { StatusPill } from './StatusPill';

type TopBarProps = {
    link: Link;
    title: string | null;
    onOpenConnection: () => void;
};

export const TopBar = ({ link, title, onOpenConnection }: TopBarProps) => {
    const t = useT();

    return (
        <header className="remote-glass-header sticky top-0 z-20 border-b px-3.5 pb-2.5 pt-3">
            <div className="flex items-center gap-2.5">
                <IconButton
                    size="lg"
                    shrink
                    label={t('connection-title')}
                    icon="menu"
                    onClick={onOpenConnection}
                />
                <div className="min-w-0 flex-1">
                    <div className="font-mono text-[9.5px] font-bold tracking-[0.16em] text-(--deck-fg-4)">
                        {t('panel-brand')}
                    </div>
                    <div className="mt-0.5 min-w-0 truncate text-sm font-semibold text-(--deck-fg)">
                        {title ?? t('panel-no-game')}
                    </div>
                </div>
                <StatusPill link={link} />
            </div>
        </header>
    );
};

import { useT } from '@/i18n/texts';
import type { Link } from '@/session/state';
import { IconButton } from '@/shared/ui/IconButton';

import { StatusPill } from './StatusPill';

type ConnectionDrawerProps = {
    link: Link;
    onClose: () => void;
};

/** The link to the app, read only: pairing, language and accent are the app's. */
export const ConnectionDrawer = ({ link, onClose }: ConnectionDrawerProps) => {
    const t = useT();

    return (
        <div className="flex h-full flex-col">
            <header className="remote-glass-header flex items-center justify-between border-b px-3.5 py-3.5">
                <div className="min-w-0">
                    <h2 className="text-lg font-bold text-(--deck-fg)">{t('connection-title')}</h2>
                    <p className="mt-0.5 truncate font-mono text-[11px] text-(--deck-fg-4)">
                        {location.host}
                    </p>
                </div>
                <IconButton label={t('connection-close')} icon="x" onClick={onClose} />
            </header>
            <div className="space-y-3 p-3.5 text-[12.5px] leading-5 text-(--deck-fg-3)">
                <StatusPill link={link} />
                <p>{t('connection-pairing')}</p>
                <p>{t('connection-follows')}</p>
            </div>
        </div>
    );
};

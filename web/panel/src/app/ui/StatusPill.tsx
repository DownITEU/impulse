import { useT } from '@/i18n/texts';
import type { Link } from '@/session/state';
import { cn } from '@/shared/lib/ui';
import { Icon } from '@/shared/ui/Icon';

const TONES: Record<Link, string> = {
    open: 'border-[color-mix(in_oklab,var(--deck-accent)_30%,transparent)] text-(--deck-accent)',
    connecting: 'border-amber-300/30 text-amber-300',
    lost: 'border-white/10 text-(--deck-fg-4)',
};

/** The socket to the app. */
export const StatusPill = ({ link }: { link: Link }) => {
    const t = useT();
    const caption = () => {
        switch (link) {
            case 'open':
                return t('link-open');
            case 'connecting':
                return t('link-connecting');
            case 'lost':
                return t('link-lost');
        }
    };

    return (
        <div
            className={cn(
                'inline-flex items-center gap-1.5 rounded-full border bg-white/4 px-2.5 py-1 font-mono text-[9.5px] font-bold tracking-[0.12em] backdrop-blur-md',
                TONES[link],
            )}
        >
            {link === 'lost' ? (
                <Icon className="size-3" name="alert" />
            ) : (
                <span className="size-1.5 rounded-full bg-current shadow-[0_0_6px_currentColor] motion-safe:animate-[breathe_1.6s_ease-in-out_infinite]" />
            )}
            {caption()}
        </div>
    );
};

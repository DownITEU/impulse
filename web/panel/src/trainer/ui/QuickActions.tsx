import { type FormEvent, useEffect, useRef, useState } from 'react';

import type { Collection } from '@/api';
import { useT } from '@/i18n/texts';
import { send } from '@/session/store';
import { cn } from '@/shared/lib/ui';
import { Icon, type IconName } from '@/shared/ui/Icon';
import { IconButton } from '@/shared/ui/IconButton';

const CHIP =
    'inline-flex shrink-0 items-center gap-1.5 rounded-full border px-3 py-1.5 text-[11.5px] font-semibold shadow-[inset_0_1px_0_rgba(255,255,255,0.05)] backdrop-blur-xl disabled:cursor-not-allowed disabled:opacity-40';

type QuickActionsProps = {
    sheet: number;
    collections: Collection[];
    online: boolean;
    live: boolean;
};

export const QuickActions = ({ sheet, collections, online, live }: QuickActionsProps) => {
    const t = useT();
    const [naming, setNaming] = useState(false);

    return (
        <>
            <div className="remote-scrollbar-hidden mb-3 flex gap-1.5 overflow-x-auto pb-0.5">
                <Chip
                    icon="bolt"
                    label={t('actions-reset')}
                    danger
                    disabled={!online}
                    onClick={() => send({ type: 'reset', sheet })}
                />
                {collections.map((collection, index) => (
                    <span
                        key={index}
                        className="inline-flex shrink-0 overflow-hidden rounded-full border border-white/10 bg-white/5.5 text-[11.5px] font-semibold text-(--deck-fg-2) shadow-[inset_0_1px_0_rgba(255,255,255,0.05)] backdrop-blur-xl"
                    >
                        <button
                            type="button"
                            disabled={!live}
                            className="inline-flex h-7.5 max-w-35 items-center gap-1.5 px-3 disabled:opacity-40"
                            onClick={() => send({ type: 'apply', sheet, collection: index })}
                        >
                            <Icon
                                className="size-3 shrink-0 text-(--deck-accent)"
                                name="sparkles"
                                stroke={2}
                            />
                            <span className="truncate">{collection.name}</span>
                        </button>
                        <button
                            type="button"
                            disabled={!online}
                            aria-label={t('actions-forget', { name: collection.name })}
                            className="flex h-7.5 w-7 items-center justify-center border-l border-white/10 text-(--deck-fg-4) hover:text-(--deck-fg) disabled:opacity-40"
                            onClick={() => send({ type: 'forget', sheet, collection: index })}
                        >
                            <Icon className="size-3" name="x" stroke={2.1} />
                        </button>
                    </span>
                ))}
                <Chip
                    icon="plus"
                    label={t('actions-collect')}
                    disabled={!online}
                    onClick={() => setNaming(true)}
                />
            </div>
            {naming ? (
                <NameDialog
                    onClose={() => setNaming(false)}
                    onSave={(name) => {
                        send({ type: 'collect', sheet, name });
                        setNaming(false);
                    }}
                />
            ) : null}
        </>
    );
};

type ChipProps = {
    icon: IconName;
    label: string;
    danger?: boolean;
    disabled: boolean;
    onClick: () => void;
};

const Chip = ({ icon, label, danger = false, disabled, onClick }: ChipProps) => (
    <button
        type="button"
        disabled={disabled}
        className={cn(
            CHIP,
            danger
                ? 'border-red-400/30 bg-red-500/10 text-red-300'
                : 'border-white/10 bg-white/5.5 text-(--deck-fg-2)',
        )}
        onClick={onClick}
    >
        <Icon className="size-3" name={icon} stroke={2} />
        {label}
    </button>
);

/** The new collection's name: the app keeps the mods that are on. */
const NameDialog = ({
    onClose,
    onSave,
}: {
    onClose: () => void;
    onSave: (name: string) => void;
}) => {
    const t = useT();
    const dialog = useRef<HTMLDialogElement | null>(null);
    const [name, setName] = useState('');
    const trimmed = name.trim();

    useEffect(() => {
        const opener = document.activeElement as HTMLElement | null;
        dialog.current?.showModal();
        return () => opener?.focus();
    }, []);

    const submit = (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        if (trimmed) onSave(trimmed);
    };

    return (
        // biome-ignore lint/a11y/useKeyWithClickEvents: the click is the backdrop's; Escape (onCancel) is the keyboard's.
        <dialog
            ref={dialog}
            aria-label={t('collect-title')}
            className="mx-auto mb-3 mt-auto w-[calc(100%-1.5rem)] max-w-90.5 bg-transparent text-(--deck-fg) backdrop:bg-black/45 backdrop:backdrop-blur-[6px] sm:mb-auto"
            onCancel={(event) => {
                event.preventDefault();
                onClose();
            }}
            // A click on the dialog itself, not its form, is on the backdrop.
            onClick={(event) => event.target === event.currentTarget && onClose()}
        >
            <form
                className="remote-glass-drawer w-full rounded-[18px] border border-white/10 p-4"
                onSubmit={submit}
            >
                <div className="mb-3 flex items-start justify-between gap-3">
                    <div className="min-w-0">
                        <h3 className="text-[15px] font-bold text-(--deck-fg)">
                            {t('collect-title')}
                        </h3>
                        <p className="mt-1 text-[11.5px] leading-snug text-(--deck-fg-3)">
                            {t('collect-tip')}
                        </p>
                    </div>
                    <IconButton shrink label={t('collect-close')} icon="x" onClick={onClose} />
                </div>
                <label className="mb-3 block">
                    <span className="mb-1.5 block font-mono text-[10px] font-bold uppercase tracking-[0.16em] text-(--deck-fg-4)">
                        {t('collect-name')}
                    </span>
                    <input
                        autoFocus
                        value={name}
                        spellCheck={false}
                        className="remote-glass-control h-11 w-full rounded-[10px] border px-3 text-[13px] font-semibold text-(--deck-fg) outline-none"
                        onInput={(event) => setName(event.currentTarget.value)}
                    />
                </label>
                <div className="grid grid-cols-2 gap-2">
                    <button
                        type="button"
                        className="remote-glass-control h-10 rounded-[10px] border text-[12px] font-semibold text-(--deck-fg-2)"
                        onClick={onClose}
                    >
                        {t('collect-cancel')}
                    </button>
                    <button
                        type="submit"
                        disabled={!trimmed}
                        className="h-10 rounded-[10px] bg-(--deck-accent) text-[12px] font-bold text-black shadow-[0_8px_24px_-8px_var(--deck-accent)] disabled:opacity-40"
                    >
                        {t('collect-save')}
                    </button>
                </div>
            </form>
        </dialog>
    );
};
